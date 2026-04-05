@echo off
setlocal EnableDelayedExpansion

:: ============================================================
::  install.bat — Compile, installe et démarre MonitoringAlert
::
::  Usage :
::    install.bat                            DB par défaut (ProgramData)
::    install.bat "D:\Backup\temps.db"       DB personnalisée
::
::  Après installation, tous les fichiers sont dans :
::    C:\Program Files\MonitoringAlert\      — executable
::    C:\ProgramData\MonitoringAlert\        — config.toml, DB, scripts
:: ============================================================

:: --- Vérification des droits admin ---
net session >nul 2>&1
if %errorLevel% neq 0 (
    echo [ERREUR] Ce script doit etre execute en tant qu'Administrateur.
    exit /b 1
)

set "INSTALL_DIR=C:\Program Files\MonitoringAlert"
set "DATA_DIR=C:\ProgramData\MonitoringAlert"
set "EXE_NAME=monitoring-alert.exe"
set "TARGET=x86_64-pc-windows-msvc"

:: Récupérer le dossier contenant ce script (source des .bat)
set "SCRIPT_DIR=%~dp0"

:: --- 1. Build ---
echo [1/6] Compilation release...
cargo build --release --target %TARGET%
if %errorLevel% neq 0 (
    echo [ERREUR] La compilation a echoue.
    exit /b 1
)

:: --- 2. Copie de l'executable ---
echo [2/6] Installation de l'executable dans "%INSTALL_DIR%"...
if not exist "%INSTALL_DIR%" mkdir "%INSTALL_DIR%"
copy /Y "target\%TARGET%\release\%EXE_NAME%" "%INSTALL_DIR%\%EXE_NAME%" >nul
if %errorLevel% neq 0 (
    echo [ERREUR] Impossible de copier l'executable.
    exit /b 1
)

:: --- 3. Creation du dossier de donnees ---
echo [3/6] Creation du dossier de donnees "%DATA_DIR%"...
if not exist "%DATA_DIR%" mkdir "%DATA_DIR%"

:: --- 4. Copie des scripts de gestion dans ProgramData ---
echo [4/6] Copie des scripts de gestion dans "%DATA_DIR%"...
copy /Y "%SCRIPT_DIR%install.bat"   "%DATA_DIR%\install.bat"   >nul
copy /Y "%SCRIPT_DIR%uninstall.bat" "%DATA_DIR%\uninstall.bat" >nul
copy /Y "%SCRIPT_DIR%update.bat"    "%DATA_DIR%\update.bat"    >nul
echo        Scripts disponibles dans %DATA_DIR%

:: --- 5. Configuration du chemin DB ---
if not "%~1"=="" (
    echo [5/6] Ecriture de config.toml avec le chemin DB personnalise...
    powershell -NoProfile -Command ^
        "$p = '%~1' -replace '\\', '\\\\'; " ^
        "[System.IO.File]::WriteAllText('%DATA_DIR%\config.toml', \"db_path = \`\"`$p\`\"\`n\")"
    if %errorLevel% neq 0 (
        echo [ERREUR] Impossible d'ecrire config.toml.
        exit /b 1
    )
    echo        DB : %~1
) else (
    echo [5/6] Aucun chemin DB personnalise -- DB par defaut : %DATA_DIR%\temperatures.db
)

:: --- 6. Enregistrement et demarrage du service ---
echo [6/6] Enregistrement du service Windows...
"%INSTALL_DIR%\%EXE_NAME%" service install
if %errorLevel% neq 0 (
    echo [ERREUR] L'enregistrement du service a echoue.
    exit /b 1
)
"%INSTALL_DIR%\%EXE_NAME%" service start
if %errorLevel% neq 0 (
    echo [ERREUR] Le demarrage du service a echoue.
    exit /b 1
)

echo.
echo Installation terminee. Le service MonitoringAlert est actif.
echo.
echo  Fichiers installes :
echo    Executable : %INSTALL_DIR%\%EXE_NAME%
echo    Donnees    : %DATA_DIR%\
echo      - config.toml   (chemin DB)
echo      - temperatures.db (creee au 1er demarrage du service)
echo      - install.bat / uninstall.bat / update.bat
echo.
echo  Verification : sc query MonitoringAlert
echo              ou services.msc

endlocal
