@echo off
setlocal EnableDelayedExpansion

:: ============================================================
::  apply-config.bat — Applique les modifications de config.toml
::
::  A lancer en tant qu'Administrateur apres avoir edite :
::    %LOCALAPPDATA%\Programs\MonitoringAlert\config.toml
::
::  Ce script :
::    1. Relit config.toml
::    2. Redémarre le service Windows (collecte)
::    3. Resynchronise les taches planifiees (rapports)
:: ============================================================

:: --- Vérification des droits admin ---
net session >nul 2>&1
if %errorLevel% neq 0 (
    echo [ERREUR] Ce script doit etre execute en tant qu'Administrateur.
    pause
    exit /b 1
)

set "DATA_DIR=%LOCALAPPDATA%\Programs\MonitoringAlert"
set "EXE_NAME=monitoring-alert.exe"
set "CONFIG_FILE=%DATA_DIR%\config.toml"

:: --- Vérification de config.toml ---
if not exist "%CONFIG_FILE%" (
    echo [ERREUR] config.toml introuvable dans %DATA_DIR%
    echo         Verifiez que MonitoringAlert est bien installe.
    pause
    exit /b 1
)

:: --- Lecture de install_dir ---
for /f "usebackq delims=" %%v in (`powershell -NoProfile -Command "$q=[char]34;$c=gc '%CONFIG_FILE%' -Raw;$null=$c-match('install_dir\s*=\s*'+$q+'([^'+$q+']+)'+$q);$matches[1]"`) do set "INSTALL_DIR=%%v"
if "!INSTALL_DIR!"=="" set "INSTALL_DIR=C:\Program Files\MonitoringAlert"

echo  Configuration : %CONFIG_FILE%
echo  Executable    : !INSTALL_DIR!\%EXE_NAME%
echo.

:: --- 1. Redémarrage du service ---
echo [1/2] Redemarrage du service...
"!INSTALL_DIR!\%EXE_NAME%" service stop >nul 2>&1
timeout /t 2 /nobreak >nul
"!INSTALL_DIR!\%EXE_NAME%" service start
if %errorLevel% neq 0 (
    echo [AVERTISSEMENT] Le redemarrage du service a echoue. Verifiez les logs.
)

:: --- 2. Resynchronisation des tâches planifiées ---
echo [2/2] Resynchronisation des taches planifiees...

powershell -NoProfile -ExecutionPolicy Bypass -File "%DATA_DIR%\Register-Tasks.ps1" -ExePath "!INSTALL_DIR!\%EXE_NAME%" -Username "%USERNAME%"

echo.
echo Configuration appliquee.
echo.
pause

endlocal
