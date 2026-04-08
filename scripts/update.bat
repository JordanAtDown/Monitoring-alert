@echo off
setlocal EnableDelayedExpansion

:: ============================================================
::  update.bat — Met a jour MonitoringAlert depuis GitHub Releases
::
::  Lit le dossier d'installation depuis :
::    C:\ProgramData\MonitoringAlert\config.toml  (cle install_dir)
::
::  Le fichier config.toml et la base de donnees sont preserves.
::  Les taches planifiees sont resynchronisees avec la config.
:: ============================================================

:: --- Vérification des droits admin ---
net session >nul 2>&1
if %errorLevel% neq 0 (
    echo [ERREUR] Ce script doit etre execute en tant qu'Administrateur.
    pause
    exit /b 1
)

set "DATA_DIR=C:\ProgramData\MonitoringAlert"
set "EXE_NAME=monitoring-alert.exe"
set "CONFIG_FILE=%DATA_DIR%\config.toml"
set "TMP_EXE=%TEMP%\monitoring-alert-update.exe"
set "API_URL=https://api.github.com/repos/jordanatdown/monitoring-alert/releases/latest"

:: --- Lecture de install_dir depuis config.toml ---
if not exist "%CONFIG_FILE%" (
    echo [AVERTISSEMENT] config.toml introuvable dans %DATA_DIR%
    echo                 Utilisation du chemin par defaut.
    set "INSTALL_DIR=C:\Program Files\MonitoringAlert"
) else (
    for /f "usebackq delims=" %%v in (`powershell -NoProfile -Command "$q=[char]34;$c=gc '%CONFIG_FILE%' -Raw;$null=$c-match('install_dir\s*=\s*'+$q+'([^'+$q+']+)'+$q);$matches[1]"`) do set "INSTALL_DIR=%%v"
)

if "!INSTALL_DIR!"=="" set "INSTALL_DIR=C:\Program Files\MonitoringAlert"
echo  Dossier d'installation : !INSTALL_DIR!
echo.

:: --- 1. Arrêt du service ---
echo [1/5] Arret du service...
"!INSTALL_DIR!\%EXE_NAME%" service stop >nul 2>&1
timeout /t 3 /nobreak >nul

:: --- 2. Récupération de l'URL du binaire depuis GitHub ---
echo [2/5] Recuperation de la derniere release sur GitHub...
for /f "usebackq delims=" %%u in (`powershell -NoProfile -Command "$q=[char]34;(Invoke-RestMethod -Uri '%API_URL%').assets | Where-Object { $_.name -eq 'monitoring-alert.exe' } | Select-Object -ExpandProperty browser_download_url"`) do set "DOWNLOAD_URL=%%u"

if "!DOWNLOAD_URL!"=="" (
    echo [ERREUR] Impossible de trouver l'asset 'monitoring-alert.exe' dans la derniere release.
    echo         Verifiez que la release GitHub contient bien cet asset.
    goto :resync
)
echo        URL : !DOWNLOAD_URL!

:: --- 3. Téléchargement et remplacement de l'exécutable ---
echo [3/5] Telechargement...
curl -L --fail --silent --show-error -o "%TMP_EXE%" "!DOWNLOAD_URL!"
if %errorLevel% neq 0 (
    echo [ERREUR] Le telechargement a echoue.
    goto :resync
)
copy /Y "%TMP_EXE%" "!INSTALL_DIR!\%EXE_NAME%" >nul
del /f /q "%TMP_EXE%" >nul 2>&1
echo        Executable mis a jour dans !INSTALL_DIR!

:: --- 4. Redémarrage du service ---
:resync
echo [4/5] Redemarrage du service...
"!INSTALL_DIR!\%EXE_NAME%" service start
if %errorLevel% neq 0 (
    echo [ERREUR] Le redemarrage du service a echoue. Verifiez les logs.
)

:: --- 5. Resynchronisation des tâches planifiées ---
echo [5/5] Resynchronisation des taches planifiees depuis config.toml...

for /f "usebackq delims=" %%v in (`powershell -NoProfile -Command "$c=gc '%CONFIG_FILE%' -Raw;if($c-match 'daily_report_enabled\s*=\s*(\w+)'){$matches[1]}else{'true'}"`) do set "DAILY_ENABLED=%%v"
for /f "usebackq delims=" %%v in (`powershell -NoProfile -Command "$q=[char]34;$c=gc '%CONFIG_FILE%' -Raw;if($c-match('daily_report_time\s*=\s*'+$q+'([^'+$q+']+)'+$q)){$matches[1]}else{'08:00'}"`) do set "DAILY_TIME=%%v"
for /f "usebackq delims=" %%v in (`powershell -NoProfile -Command "$c=gc '%CONFIG_FILE%' -Raw;if($c-match 'weekly_report_enabled\s*=\s*(\w+)'){$matches[1]}else{'false'}"`) do set "WEEKLY_ENABLED=%%v"
for /f "usebackq delims=" %%v in (`powershell -NoProfile -Command "$q=[char]34;$c=gc '%CONFIG_FILE%' -Raw;if($c-match('weekly_report_day\s*=\s*'+$q+'([^'+$q+']+)'+$q)){$matches[1]}else{'MON'}"`) do set "WEEKLY_DAY=%%v"
for /f "usebackq delims=" %%v in (`powershell -NoProfile -Command "$q=[char]34;$c=gc '%CONFIG_FILE%' -Raw;if($c-match('weekly_report_time\s*=\s*'+$q+'([^'+$q+']+)'+$q)){$matches[1]}else{'08:00'}"`) do set "WEEKLY_TIME=%%v"
for /f "usebackq delims=" %%v in (`powershell -NoProfile -Command "$c=gc '%CONFIG_FILE%' -Raw;if($c-match 'monthly_report_enabled\s*=\s*(\w+)'){$matches[1]}else{'false'}"`) do set "MONTHLY_ENABLED=%%v"
for /f "usebackq delims=" %%v in (`powershell -NoProfile -Command "$c=gc '%CONFIG_FILE%' -Raw;if($c-match 'monthly_report_day\s*=\s*(\d+)'){$matches[1]}else{'1'}"`) do set "MONTHLY_DAY=%%v"
for /f "usebackq delims=" %%v in (`powershell -NoProfile -Command "$q=[char]34;$c=gc '%CONFIG_FILE%' -Raw;if($c-match('monthly_report_time\s*=\s*'+$q+'([^'+$q+']+)'+$q)){$matches[1]}else{'08:00'}"`) do set "MONTHLY_TIME=%%v"

powershell -NoProfile -ExecutionPolicy Bypass -File "%DATA_DIR%\Register-Tasks.ps1" ^
    -ExePath "!INSTALL_DIR!\%EXE_NAME%" ^
    -Username "%USERNAME%" ^
    -DailyEnabled "!DAILY_ENABLED!"   -DailyTime "!DAILY_TIME!" ^
    -WeeklyEnabled "!WEEKLY_ENABLED!" -WeeklyDay "!WEEKLY_DAY!" -WeeklyTime "!WEEKLY_TIME!" ^
    -MonthlyEnabled "!MONTHLY_ENABLED!" -MonthlyDay "!MONTHLY_DAY!" -MonthlyTime "!MONTHLY_TIME!"

echo.
echo Mise a jour terminee.
echo.
pause

endlocal
