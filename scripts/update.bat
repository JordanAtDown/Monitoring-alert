@echo off
setlocal EnableDelayedExpansion

:: ============================================================
::  update.bat — Met a jour monitoring-alert depuis GitHub Releases
::
::  Telecharge la derniere release et lance son installation : config.toml
::  et la base de donnees sont conserves.
::
::  A lancer en tant qu'Administrateur si une version 1.x est encore installee.
:: ============================================================

:: L'installation remplace update.bat lui-meme : un script batch ne doit jamais
:: etre ecrase pendant qu'il s'execute. On se relance donc depuis un dossier temporaire.
if /i not "%~1"=="run" (
    copy /Y "%~f0" "%TEMP%\ma-update.bat" >nul
    call "%TEMP%\ma-update.bat" run
    set "RC=!errorlevel!"
    exit /b !RC!
)

set "TMP_ZIP=%TEMP%\ma-update.zip"
set "TMP_DIR=%TEMP%\ma-update"
set "API_URL=https://api.github.com/repos/jordanatdown/monitoring-alert/releases/latest"

echo [1/3] Recuperation de la derniere release sur GitHub...
for /f "usebackq delims=" %%u in (`powershell -NoProfile -Command "(Invoke-RestMethod -Uri '%API_URL%').assets | Where-Object { $_.name -like '*.zip' } | Select-Object -ExpandProperty browser_download_url -First 1"`) do set "ZIP_URL=%%u"
if "!ZIP_URL!"=="" (
    echo [ERREUR] Impossible de trouver l'archive zip dans la derniere release.
    pause
    exit /b 1
)
echo        !ZIP_URL!

echo [2/3] Telechargement et extraction...
if exist "%TMP_DIR%" rd /s /q "%TMP_DIR%"
curl -L --fail --silent --show-error -o "%TMP_ZIP%" "!ZIP_URL!"
if errorlevel 1 (
    echo [ERREUR] Le telechargement a echoue.
    pause
    exit /b 1
)
powershell -NoProfile -Command "Expand-Archive -Path '%TMP_ZIP%' -DestinationPath '%TMP_DIR%' -Force"
if errorlevel 1 (
    echo [ERREUR] L'extraction du zip a echoue.
    pause
    exit /b 1
)
for /f "usebackq delims=" %%d in (`powershell -NoProfile -Command "Get-ChildItem '%TMP_DIR%' -Directory | Select-Object -ExpandProperty FullName -First 1"`) do set "EXTRACTED=%%d"
if "!EXTRACTED!"=="" (
    echo [ERREUR] Impossible de trouver le contenu extrait.
    pause
    exit /b 1
)

echo [3/3] Installation...
"!EXTRACTED!\monitoring-alert.exe" install
set "RC=!errorlevel!"

del /f /q "%TMP_ZIP%" >nul 2>&1
rd /s /q "%TMP_DIR%" >nul 2>&1

echo.
if not "!RC!"=="0" echo [ERREUR] L'installation a echoue. Voir le message ci-dessus.
pause
exit /b !RC!
