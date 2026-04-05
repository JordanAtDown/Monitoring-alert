@echo off
setlocal EnableDelayedExpansion

:: ============================================================
::  update.bat — Met a jour MonitoringAlert depuis GitHub Releases
::
::  Telechargement de la derniere release de :
::    https://github.com/jordanatdown/monitoring-alert
::
::  Le chemin d'installation est lu depuis :
::    C:\ProgramData\MonitoringAlert\install_path.txt
::
::  Le fichier config.toml et la base de donnees sont preserves.
:: ============================================================

:: --- Vérification des droits admin ---
net session >nul 2>&1
if %errorLevel% neq 0 (
    echo [ERREUR] Ce script doit etre execute en tant qu'Administrateur.
    exit /b 1
)

set "DATA_DIR=C:\ProgramData\MonitoringAlert"
set "EXE_NAME=monitoring-alert.exe"
set "DEFAULT_INSTALL_DIR=C:\Program Files\MonitoringAlert"
set "TMP_EXE=%TEMP%\monitoring-alert-update.exe"
set "API_URL=https://api.github.com/repos/jordanatdown/monitoring-alert/releases/latest"

:: --- Lecture du chemin d'installation depuis install_path.txt ---
if exist "%DATA_DIR%\install_path.txt" (
    set /p INSTALL_DIR=<"%DATA_DIR%\install_path.txt"
    echo  Chemin d'installation lu : !INSTALL_DIR!
) else (
    set "INSTALL_DIR=%DEFAULT_INSTALL_DIR%"
    echo  install_path.txt introuvable -- utilisation du chemin par defaut : %DEFAULT_INSTALL_DIR%
)
echo.

:: --- 1. Arrêt du service ---
echo [1/4] Arret du service...
"!INSTALL_DIR!\%EXE_NAME%" service stop >nul 2>&1
timeout /t 3 /nobreak >nul

:: --- 2. Récupération de l'URL du binaire ---
echo [2/4] Recuperation de la derniere release sur GitHub...
for /f "usebackq delims=" %%u in (
    `powershell -NoProfile -Command ^
        "(Invoke-RestMethod -Uri '%API_URL%').assets | Where-Object { $_.name -eq 'monitoring-alert.exe' } | Select-Object -ExpandProperty browser_download_url"`
) do set "DOWNLOAD_URL=%%u"

if "!DOWNLOAD_URL!"=="" (
    echo [ERREUR] Impossible de trouver l'asset 'monitoring-alert.exe' dans la derniere release.
    echo         Verifiez que la release GitHub contient bien cet asset.
    goto :restart
)
echo        URL : !DOWNLOAD_URL!

:: --- 3. Téléchargement et remplacement ---
echo [3/4] Telechargement...
curl -L --fail --silent --show-error -o "%TMP_EXE%" "!DOWNLOAD_URL!"
if %errorLevel% neq 0 (
    echo [ERREUR] Le telechargement a echoue.
    goto :restart
)
copy /Y "%TMP_EXE%" "!INSTALL_DIR!\%EXE_NAME%" >nul
del /f /q "%TMP_EXE%" >nul 2>&1
echo        Executable mis a jour dans !INSTALL_DIR!

:: --- 4. Redémarrage du service ---
:restart
echo [4/4] Redemarrage du service...
"!INSTALL_DIR!\%EXE_NAME%" service start
if %errorLevel% neq 0 (
    echo [ERREUR] Le redemarrage du service a echoue. Verifiez les logs.
    exit /b 1
)

echo.
echo Mise a jour terminee.

endlocal
