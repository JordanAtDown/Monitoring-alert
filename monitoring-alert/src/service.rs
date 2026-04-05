use anyhow::Result;

#[cfg(windows)]
pub const SERVICE_NAME: &str = "MonitoringAlert";
#[cfg(windows)]
pub const SERVICE_DISPLAY_NAME: &str = "Monitoring Alert - Temperature Monitor";
#[cfg(windows)]
pub const SERVICE_DESCRIPTION: &str =
    "Collecte les températures système toutes les 5 minutes via LibreHardwareMonitor";

#[cfg(windows)]
pub mod windows {
    use super::*;
    use anyhow::{bail, Context};
    use std::ffi::OsString;
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::Arc;
    use std::time::Duration;
    use windows_service::{
        service::{
            ServiceAccess, ServiceAction, ServiceActionType, ServiceErrorControl,
            ServiceFailureActions, ServiceFailureResetPeriod, ServiceInfo, ServiceStartType,
            ServiceState, ServiceStatus, ServiceType,
        },
        service_control_handler::{self, ServiceControlHandlerResult},
        service_manager::{ServiceManager, ServiceManagerAccess},
    };

    pub fn install() -> Result<()> {
        let manager =
            ServiceManager::local_computer(None::<&str>, ServiceManagerAccess::CREATE_SERVICE)
                .context("Failed to open Service Manager (try running as administrator)")?;

        let exe = std::env::current_exe().context("Failed to get current executable path")?;

        let service_info = ServiceInfo {
            name: OsString::from(SERVICE_NAME),
            display_name: OsString::from(SERVICE_DISPLAY_NAME),
            service_type: ServiceType::OWN_PROCESS,
            start_type: ServiceStartType::AutoStart,
            error_control: ServiceErrorControl::Normal,
            executable_path: exe,
            launch_arguments: vec![],
            dependencies: vec![],
            account_name: None,
            account_password: None,
        };

        let service = manager
            .create_service(
                &service_info,
                ServiceAccess::CHANGE_CONFIG | ServiceAccess::START,
            )
            .context("Failed to create service")?;

        // Set description
        service
            .set_description(SERVICE_DESCRIPTION)
            .context("Failed to set service description")?;

        // Set recovery actions: restart after 5s, 3 times
        let recovery_actions = vec![
            ServiceAction {
                action_type: ServiceActionType::Restart,
                delay: Duration::from_secs(5),
            },
            ServiceAction {
                action_type: ServiceActionType::Restart,
                delay: Duration::from_secs(5),
            },
            ServiceAction {
                action_type: ServiceActionType::Restart,
                delay: Duration::from_secs(5),
            },
        ];
        let failure_actions = ServiceFailureActions {
            reset_period: ServiceFailureResetPeriod::After(Duration::from_secs(86400)),
            reboot_msg: None,
            command: None,
            actions: Some(recovery_actions),
        };
        service
            .update_failure_actions(failure_actions)
            .context("Failed to set failure/recovery actions")?;

        println!("Service '{}' installed successfully.", SERVICE_DISPLAY_NAME);
        println!("Run 'monitoring-alert service start' to start it.");
        Ok(())
    }

    pub fn uninstall() -> Result<()> {
        let manager = ServiceManager::local_computer(None::<&str>, ServiceManagerAccess::CONNECT)
            .context("Failed to open Service Manager")?;
        let service = manager
            .open_service(SERVICE_NAME, ServiceAccess::DELETE | ServiceAccess::STOP)
            .context("Failed to open service (is it installed?)")?;

        // Stop it first if running
        let status = service
            .query_status()
            .context("Failed to query service status")?;
        if status.current_state != ServiceState::Stopped {
            service
                .stop()
                .context("Failed to stop service before uninstalling")?;
            std::thread::sleep(Duration::from_secs(2));
        }

        service.delete().context("Failed to delete service")?;
        println!("Service '{}' uninstalled.", SERVICE_NAME);
        Ok(())
    }

    pub fn start() -> Result<()> {
        let manager = ServiceManager::local_computer(None::<&str>, ServiceManagerAccess::CONNECT)
            .context("Failed to open Service Manager")?;
        let service = manager
            .open_service(SERVICE_NAME, ServiceAccess::START)
            .context("Failed to open service")?;
        service
            .start::<&str>(&[])
            .context("Failed to start service")?;
        println!("Service '{}' started.", SERVICE_NAME);
        Ok(())
    }

    pub fn stop() -> Result<()> {
        let manager = ServiceManager::local_computer(None::<&str>, ServiceManagerAccess::CONNECT)
            .context("Failed to open Service Manager")?;
        let service = manager
            .open_service(SERVICE_NAME, ServiceAccess::STOP)
            .context("Failed to open service")?;
        service.stop().context("Failed to stop service")?;
        println!("Service '{}' stop signal sent.", SERVICE_NAME);
        Ok(())
    }

    /// Called by `ffi_service_main` — runs the actual service logic.
    pub fn run_service_main(_args: Vec<OsString>) -> Result<()> {
        let stop_flag = Arc::new(AtomicBool::new(false));
        let stop_flag_handler = Arc::clone(&stop_flag);

        let event_handler = move |control_event| -> ServiceControlHandlerResult {
            match control_event {
                windows_service::service::ServiceControl::Stop
                | windows_service::service::ServiceControl::Shutdown => {
                    stop_flag_handler.store(true, Ordering::SeqCst);
                    ServiceControlHandlerResult::NoError
                }
                windows_service::service::ServiceControl::Interrogate => {
                    ServiceControlHandlerResult::NoError
                }
                _ => ServiceControlHandlerResult::NotImplemented,
            }
        };

        let status_handle = service_control_handler::register(SERVICE_NAME, event_handler)
            .context("Failed to register service control handler")?;

        // Report Running
        status_handle
            .set_service_status(ServiceStatus {
                service_type: ServiceType::OWN_PROCESS,
                current_state: ServiceState::Running,
                controls_accepted: windows_service::service::ServiceControlAccept::STOP
                    | windows_service::service::ServiceControlAccept::SHUTDOWN,
                exit_code: windows_service::service::ServiceExitCode::Win32(0),
                checkpoint: 0,
                wait_hint: Duration::default(),
                process_id: None,
            })
            .context("Failed to set service status Running")?;

        let db_path = crate::config::AppConfig::load().db_path;

        let result = crate::collector::watch(&db_path, 300, stop_flag);

        // Report Stopped
        let exit_code = match &result {
            Ok(_) => windows_service::service::ServiceExitCode::Win32(0),
            Err(_) => windows_service::service::ServiceExitCode::ServiceSpecific(1),
        };
        let _ = status_handle.set_service_status(ServiceStatus {
            service_type: ServiceType::OWN_PROCESS,
            current_state: ServiceState::Stopped,
            controls_accepted: windows_service::service::ServiceControlAccept::empty(),
            exit_code,
            checkpoint: 0,
            wait_hint: Duration::default(),
            process_id: None,
        });

        result
    }
}

// Platform-agnostic stubs for non-Windows builds
#[cfg(not(windows))]
pub fn install() -> Result<()> {
    anyhow::bail!("Windows service management is only supported on Windows")
}
#[cfg(not(windows))]
pub fn uninstall() -> Result<()> {
    anyhow::bail!("Windows service management is only supported on Windows")
}
#[cfg(not(windows))]
pub fn start() -> Result<()> {
    anyhow::bail!("Windows service management is only supported on Windows")
}
#[cfg(not(windows))]
pub fn stop() -> Result<()> {
    anyhow::bail!("Windows service management is only supported on Windows")
}

#[cfg(windows)]
pub use windows::{install, start, stop, uninstall};
