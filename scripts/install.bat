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
::  update.bat dans C:\ProgramData\MonitoringAlert\
:: ============================================================

:: --- Vérification des droits admin ---
net session >nul 2>&1
if %errorLevel% neq 0 (
    echo [ERREUR] Ce script doit etre execute en tant qu'Administrateur.
    exit /b 1
)

set "SCRIPT_DIR=%~dp0"
set "DATA_DIR=C:\ProgramData\MonitoringAlert"
set "EXE_NAME=monitoring-alert.exe"
set "CONFIG_FILE=%SCRIPT_DIR%config.toml"

:: --- Vérification de la présence de config.toml ---
if not exist "%CONFIG_FILE%" (
    echo [ERREUR] config.toml introuvable dans %SCRIPT_DIR%
    echo         Assurez-vous que config.toml est present dans le meme dossier que install.bat.
    exit /b 1
)

:: --- Vérification de la présence de l'exécutable ---
if not exist "%SCRIPT_DIR%%EXE_NAME%" (
    echo [ERREUR] %EXE_NAME% introuvable dans %SCRIPT_DIR%
    echo         Assurez-vous que monitoring-alert.exe est present dans le meme dossier.
    exit /b 1
)

:: --- Lecture de install_dir depuis config.toml ---
for /f "usebackq delims=" %%v in (
    `powershell -NoProfile -Command ^
        "$c = Get-Content '%CONFIG_FILE%' -Raw; " ^
        "if ($c -match 'install_dir\s*=\s*\"([^\"]+)\"') { $Matches[1] }"`
) do set "INSTALL_DIR=%%v"

if "!INSTALL_DIR!"=="" (
    echo [ERREUR] Cle 'install_dir' introuvable dans config.toml.
    exit /b 1
)

:: --- Lecture de db_path depuis config.toml ---
for /f "usebackq delims=" %%v in (
    `powershell -NoProfile -Command ^
        "$c = Get-Content '%CONFIG_FILE%' -Raw; " ^
        "if ($c -match 'db_path\s*=\s*\"([^\"]+)\"') { $Matches[1] }"`
) do set "DB_PATH=%%v"

:: --- Lecture des paramètres de rapport depuis config.toml ---
for /f "usebackq delims=" %%v in (
    `powershell -NoProfile -Command ^
        "$c = Get-Content '%CONFIG_FILE%' -Raw; " ^
        "if ($c -match 'daily_report_enabled\s*=\s*(\w+)') { $Matches[1] } else { 'true' }"`
) do set "DAILY_ENABLED=%%v"

for /f "usebackq delims=" %%v in (
    `powershell -NoProfile -Command ^
        "$c = Get-Content '%CONFIG_FILE%' -Raw; " ^
        "if ($c -match 'daily_report_time\s*=\s*""([^""]+)""') { $Matches[1] } else { '08:00' }"`
) do set "DAILY_TIME=%%v"

for /f "usebackq delims=" %%v in (
    `powershell -NoProfile -Command ^
        "$c = Get-Content '%CONFIG_FILE%' -Raw; " ^
        "if ($c -match 'weekly_report_enabled\s*=\s*(\w+)') { $Matches[1] } else { 'false' }"`
) do set "WEEKLY_ENABLED=%%v"

for /f "usebackq delims=" %%v in (
    `powershell -NoProfile -Command ^
        "$c = Get-Content '%CONFIG_FILE%' -Raw; " ^
        "if ($c -match 'weekly_report_day\s*=\s*""([^""]+)""') { $Matches[1] } else { 'MON' }"`
) do set "WEEKLY_DAY=%%v"

for /f "usebackq delims=" %%v in (
    `powershell -NoProfile -Command ^
        "$c = Get-Content '%CONFIG_FILE%' -Raw; " ^
        "if ($c -match 'weekly_report_time\s*=\s*""([^""]+)""') { $Matches[1] } else { '08:00' }"`
) do set "WEEKLY_TIME=%%v"

for /f "usebackq delims=" %%v in (
    `powershell -NoProfile -Command ^
        "$c = Get-Content '%CONFIG_FILE%' -Raw; " ^
        "if ($c -match 'monthly_report_enabled\s*=\s*(\w+)') { $Matches[1] } else { 'false' }"`
) do set "MONTHLY_ENABLED=%%v"

for /f "usebackq delims=" %%v in (
    `powershell -NoProfile -Command ^
        "$c = Get-Content '%CONFIG_FILE%' -Raw; " ^
        "if ($c -match 'monthly_report_day\s*=\s*(\d+)') { $Matches[1] } else { '1' }"`
) do set "MONTHLY_DAY=%%v"

for /f "usebackq delims=" %%v in (
    `powershell -NoProfile -Command ^
        "$c = Get-Content '%CONFIG_FILE%' -Raw; " ^
        "if ($c -match 'monthly_report_time\s*=\s*""([^""]+)""') { $Matches[1] } else { '08:00' }"`
) do set "MONTHLY_TIME=%%v"

echo  Dossier d'installation : !INSTALL_DIR!
echo  Chemin DB              : !DB_PATH!
echo.

:: --- 1. Installation de l'exécutable ---
echo [1/6] Installation de l'executable dans "!INSTALL_DIR!"...
if not exist "!INSTALL_DIR!" mkdir "!INSTALL_DIR!"
copy /Y "%SCRIPT_DIR%%EXE_NAME%" "!INSTALL_DIR!\%EXE_NAME%" >nul
if %errorLevel% neq 0 (
    echo [ERREUR] Impossible de copier l'executable.
    exit /b 1
)

:: --- 2. Création du dossier de données et copie des fichiers ---
echo [2/6] Creation du dossier de donnees "%DATA_DIR%"...
if not exist "%DATA_DIR%" mkdir "%DATA_DIR%"

echo        Copie de config.toml, uninstall.bat, update.bat...
copy /Y "%CONFIG_FILE%"                 "%DATA_DIR%\config.toml"   >nul
copy /Y "%SCRIPT_DIR%uninstall.bat"     "%DATA_DIR%\uninstall.bat" >nul
copy /Y "%SCRIPT_DIR%update.bat"        "%DATA_DIR%\update.bat"    >nul

:: --- 3. Enregistrement de l'AUMID pour les notifications toast ---
echo [3/6] Enregistrement de l'AUMID pour les notifications...
reg add "HKCU\Software\Classes\AppUserModelId\MonitoringAlert.TemperatureMonitor" /f >nul
reg add "HKCU\Software\Classes\AppUserModelId\MonitoringAlert.TemperatureMonitor" /v "DisplayName" /t REG_SZ /d "Monitoring Alert" /f >nul

:: --- 4. Enregistrement et démarrage du service ---
echo [4/6] Enregistrement du service Windows...
"!INSTALL_DIR!\%EXE_NAME%" service install
if %errorLevel% neq 0 (
    echo [ERREUR] L'enregistrement du service a echoue.
    exit /b 1
)

echo [5/6] Demarrage du service...
"!INSTALL_DIR!\%EXE_NAME%" service start
if %errorLevel% neq 0 (
    echo [ERREUR] Le demarrage du service a echoue.
    exit /b 1
)

:: --- 6. Création des tâches planifiées de rapport ---
echo [6/6] Creation des taches planifiees de rapport...
schtasks /delete /tn "MonitoringAlert\RapportJournalier"   /f >nul 2>&1
schtasks /delete /tn "MonitoringAlert\RapportHebdomadaire" /f >nul 2>&1
schtasks /delete /tn "MonitoringAlert\RapportMensuel"      /f >nul 2>&1

if /i "!DAILY_ENABLED!"=="true" (
    schtasks /create /tn "MonitoringAlert\RapportJournalier" ^
        /tr "\"!INSTALL_DIR!\%EXE_NAME%\" notify --daily" ^
        /sc DAILY /st !DAILY_TIME! /ru "%USERNAME%" /f >nul
    echo        Rapport journalier : !DAILY_TIME! chaque jour
)
if /i "!WEEKLY_ENABLED!"=="true" (
    schtasks /create /tn "MonitoringAlert\RapportHebdomadaire" ^
        /tr "\"!INSTALL_DIR!\%EXE_NAME%\" notify --weekly" ^
        /sc WEEKLY /d !WEEKLY_DAY! /st !WEEKLY_TIME! /ru "%USERNAME%" /f >nul
    echo        Rapport hebdomadaire : !WEEKLY_DAY! a !WEEKLY_TIME!
)
if /i "!MONTHLY_ENABLED!"=="true" (
    schtasks /create /tn "MonitoringAlert\RapportMensuel" ^
        /tr "\"!INSTALL_DIR!\%EXE_NAME%\" notify --monthly" ^
        /sc MONTHLY /d !MONTHLY_DAY! /st !MONTHLY_TIME! /ru "%USERNAME%" /f >nul
    echo        Rapport mensuel : jour !MONTHLY_DAY! a !MONTHLY_TIME!
)

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

endlocal
