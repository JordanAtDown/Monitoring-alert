@echo off
setlocal EnableDelayedExpansion

:: ============================================================
::  install.bat — Installe et démarre MonitoringAlert
::
::  AVANT de lancer ce script :
::    1. Editez config.toml (dans ce meme dossier)
::       - install_dir : dossier d'installation de l'executable
::       - db_path     : chemin de la base de donnees
::    2. Lancez ce script en tant qu'Administrateur
::
::  Ce script copie ensuite config.toml, uninstall.bat et
::  update.bat dans %LOCALAPPDATA%\Programs\MonitoringAlert\
:: ============================================================

:: --- Vérification des droits admin ---
net session >nul 2>&1
if %errorLevel% neq 0 (
    echo [ERREUR] Ce script doit etre execute en tant qu'Administrateur.
    echo          Clic droit sur install.bat ^> "Executer en tant qu'administrateur"
    pause
    exit /b 1
)

set "SCRIPT_DIR=%~dp0"
set "DATA_DIR=%LOCALAPPDATA%\Programs\MonitoringAlert"
set "EXE_NAME=monitoring-alert.exe"
set "CONFIG_FILE=%SCRIPT_DIR%config.toml"

:: --- Vérification de la présence de config.toml ---
if not exist "%CONFIG_FILE%" (
    echo [ERREUR] config.toml introuvable dans %SCRIPT_DIR%
    echo         Assurez-vous que config.toml est present dans le meme dossier que install.bat.
    pause
    exit /b 1
)

:: --- Vérification de la présence de l'exécutable ---
if not exist "%SCRIPT_DIR%%EXE_NAME%" (
    echo [ERREUR] %EXE_NAME% introuvable dans %SCRIPT_DIR%
    echo         Assurez-vous que monitoring-alert.exe est present dans le meme dossier.
    pause
    exit /b 1
)

:: --- Lecture de la configuration depuis config.toml ---
:: Note: [char]34 = " — evite les conflits de guillemets dans for/f
for /f "usebackq delims=" %%v in (`powershell -NoProfile -Command "$q=[char]34;$c=gc '%CONFIG_FILE%' -Raw;$null=$c-match('install_dir\s*=\s*'+$q+'([^'+$q+']+)'+$q);$matches[1]"`) do set "INSTALL_DIR=%%v"

if "!INSTALL_DIR!"=="" (
    echo [ERREUR] Cle 'install_dir' introuvable dans config.toml.
    pause
    exit /b 1
)

for /f "usebackq delims=" %%v in (`powershell -NoProfile -Command "$q=[char]34;$c=gc '%CONFIG_FILE%' -Raw;$null=$c-match('db_path\s*=\s*'+$q+'([^'+$q+']+)'+$q);$matches[1]"`) do set "DB_PATH=%%v"

for /f "usebackq delims=" %%v in (`powershell -NoProfile -Command "$c=gc '%CONFIG_FILE%' -Raw;if($c-match 'daily_report_enabled\s*=\s*(\w+)'){$matches[1]}else{'true'}"`) do set "DAILY_ENABLED=%%v"
for /f "usebackq delims=" %%v in (`powershell -NoProfile -Command "$q=[char]34;$c=gc '%CONFIG_FILE%' -Raw;if($c-match('daily_report_time\s*=\s*'+$q+'([^'+$q+']+)'+$q)){$matches[1]}else{'08:00'}"`) do set "DAILY_TIME=%%v"
for /f "usebackq delims=" %%v in (`powershell -NoProfile -Command "$c=gc '%CONFIG_FILE%' -Raw;if($c-match 'weekly_report_enabled\s*=\s*(\w+)'){$matches[1]}else{'false'}"`) do set "WEEKLY_ENABLED=%%v"
for /f "usebackq delims=" %%v in (`powershell -NoProfile -Command "$q=[char]34;$c=gc '%CONFIG_FILE%' -Raw;if($c-match('weekly_report_day\s*=\s*'+$q+'([^'+$q+']+)'+$q)){$matches[1]}else{'MON'}"`) do set "WEEKLY_DAY=%%v"
for /f "usebackq delims=" %%v in (`powershell -NoProfile -Command "$q=[char]34;$c=gc '%CONFIG_FILE%' -Raw;if($c-match('weekly_report_time\s*=\s*'+$q+'([^'+$q+']+)'+$q)){$matches[1]}else{'08:00'}"`) do set "WEEKLY_TIME=%%v"
for /f "usebackq delims=" %%v in (`powershell -NoProfile -Command "$c=gc '%CONFIG_FILE%' -Raw;if($c-match 'monthly_report_enabled\s*=\s*(\w+)'){$matches[1]}else{'false'}"`) do set "MONTHLY_ENABLED=%%v"
for /f "usebackq delims=" %%v in (`powershell -NoProfile -Command "$c=gc '%CONFIG_FILE%' -Raw;if($c-match 'monthly_report_day\s*=\s*(\d+)'){$matches[1]}else{'1'}"`) do set "MONTHLY_DAY=%%v"
for /f "usebackq delims=" %%v in (`powershell -NoProfile -Command "$q=[char]34;$c=gc '%CONFIG_FILE%' -Raw;if($c-match('monthly_report_time\s*=\s*'+$q+'([^'+$q+']+)'+$q)){$matches[1]}else{'08:00'}"`) do set "MONTHLY_TIME=%%v"

echo  Dossier d'installation : !INSTALL_DIR!
echo  Chemin DB              : !DB_PATH!
echo.

:: --- 1. Installation de l'exécutable ---
echo [1/6] Installation de l'executable dans "!INSTALL_DIR!"...
if not exist "!INSTALL_DIR!" mkdir "!INSTALL_DIR!"
copy /Y "%SCRIPT_DIR%%EXE_NAME%" "!INSTALL_DIR!\%EXE_NAME%" >nul
if %errorLevel% neq 0 (
    echo [ERREUR] Impossible de copier l'executable.
    pause
    exit /b 1
)

:: --- 2. Création du dossier de données et copie des fichiers ---
echo [2/6] Creation du dossier de donnees "%DATA_DIR%"...
if not exist "%DATA_DIR%" mkdir "%DATA_DIR%"

:: Création du dossier parent de db_path s'il est différent de DATA_DIR
if not "!DB_PATH!"=="" (
    for /f "usebackq delims=" %%p in (`powershell -NoProfile -Command "Split-Path '!DB_PATH!'"`) do (
        if not exist "%%p" mkdir "%%p"
    )
)

echo        Copie de config.toml, Register-Tasks.ps1, uninstall.bat, update.bat, apply-config.bat...
copy /Y "%CONFIG_FILE%"                    "%DATA_DIR%\config.toml"          >nul
copy /Y "%SCRIPT_DIR%Register-Tasks.ps1"   "%DATA_DIR%\Register-Tasks.ps1"   >nul
copy /Y "%SCRIPT_DIR%uninstall.bat"        "%DATA_DIR%\uninstall.bat"        >nul
copy /Y "%SCRIPT_DIR%update.bat"           "%DATA_DIR%\update.bat"           >nul
copy /Y "%SCRIPT_DIR%apply-config.bat"     "%DATA_DIR%\apply-config.bat"     >nul

:: --- 3. Enregistrement de l'AUMID pour les notifications toast ---
echo [3/6] Enregistrement de l'AUMID pour les notifications...
reg add "HKCU\Software\Classes\AppUserModelId\MonitoringAlert.TemperatureMonitor" /f >nul
reg add "HKCU\Software\Classes\AppUserModelId\MonitoringAlert.TemperatureMonitor" /v "DisplayName" /t REG_SZ /d "Monitoring Alert" /f >nul
reg add "HKCU\Software\Classes\AppUserModelId\MonitoringAlert.TemperatureMonitor" /v "IconUri" /t REG_SZ /d "!INSTALL_DIR!\%EXE_NAME%" /f >nul

:: --- 4. Enregistrement et démarrage du service ---
echo [4/6] Enregistrement du service Windows...
"!INSTALL_DIR!\%EXE_NAME%" service install
if %errorLevel% neq 0 (
    echo [ERREUR] L'enregistrement du service a echoue.
    pause
    exit /b 1
)

echo [5/6] Demarrage du service...
"!INSTALL_DIR!\%EXE_NAME%" service start
if %errorLevel% neq 0 (
    echo [ERREUR] Le demarrage du service a echoue.
    pause
    exit /b 1
)

:: --- 6. Création des tâches planifiées de rapport ---
echo [6/6] Creation des taches planifiees de rapport (StartWhenAvailable)...
powershell -NoProfile -ExecutionPolicy Bypass -File "%DATA_DIR%\Register-Tasks.ps1" ^
    -ExePath "!INSTALL_DIR!\%EXE_NAME%" ^
    -Username "%USERNAME%" ^
    -DailyEnabled "!DAILY_ENABLED!"   -DailyTime "!DAILY_TIME!" ^
    -WeeklyEnabled "!WEEKLY_ENABLED!" -WeeklyDay "!WEEKLY_DAY!" -WeeklyTime "!WEEKLY_TIME!" ^
    -MonthlyEnabled "!MONTHLY_ENABLED!" -MonthlyDay "!MONTHLY_DAY!" -MonthlyTime "!MONTHLY_TIME!"

echo.
echo Installation terminee. Le service MonitoringAlert est actif.
echo.
echo  Fichiers installes :
echo    Executable : !INSTALL_DIR!\%EXE_NAME%
echo    Donnees    : %DATA_DIR%\
echo      config.toml    (chemin DB + dossier d'installation + rapports)
echo      temperatures.db (creee au 1er demarrage du service)
echo      uninstall.bat / update.bat
echo.
echo  Verification : sc query MonitoringAlert
echo              ou services.msc
echo.
pause

endlocal
