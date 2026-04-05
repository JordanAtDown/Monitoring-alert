@echo off
setlocal EnableDelayedExpansion

:: ============================================================
::  install.bat — Compile, installe et démarre MonitoringAlert
::
::  Usage :
::    install.bat [chemin_db] [dossier_installation]
::
::  Exemples :
::    install.bat
::      → exe dans C:\Program Files\MonitoringAlert
::      → DB  dans C:\ProgramData\MonitoringAlert\temperatures.db
::
::    install.bat "D:\Backup\temps.db"
::      → exe dans C:\Program Files\MonitoringAlert
::      → DB  dans D:\Backup\temps.db
::
::    install.bat "" "D:\Apps\MonitoringAlert"
::      → exe dans D:\Apps\MonitoringAlert
::      → DB  dans C:\ProgramData\MonitoringAlert\temperatures.db
::
::    install.bat "D:\Backup\temps.db" "D:\Apps\MonitoringAlert"
::      → exe dans D:\Apps\MonitoringAlert
::      → DB  dans D:\Backup\temps.db
::
::  Après installation, tous les fichiers de gestion sont dans :
::    C:\ProgramData\MonitoringAlert\
:: ============================================================

:: --- Vérification des droits admin ---
net session >nul 2>&1
if %errorLevel% neq 0 (
    echo [ERREUR] Ce script doit etre execute en tant qu'Administrateur.
    exit /b 1
)

:: --- Chemins par défaut ---
set "DEFAULT_INSTALL_DIR=C:\Program Files\MonitoringAlert"
set "DATA_DIR=C:\ProgramData\MonitoringAlert"
set "EXE_NAME=monitoring-alert.exe"
set "TARGET=x86_64-pc-windows-msvc"

:: Récupérer le dossier contenant ce script (source des .bat)
set "SCRIPT_DIR=%~dp0"

:: Chemin DB : %1 si fourni et non vide
if not "%~1"=="" (
    set "CUSTOM_DB=%~1"
) else (
    set "CUSTOM_DB="
)

:: Dossier d'installation : %2 si fourni et non vide, sinon défaut
if not "%~2"=="" (
    set "INSTALL_DIR=%~2"
) else (
    set "INSTALL_DIR=%DEFAULT_INSTALL_DIR%"
)

echo  Dossier d'installation : %INSTALL_DIR%
if not "!CUSTOM_DB!"=="" (
    echo  Chemin DB personnalise : !CUSTOM_DB!
) else (
    echo  Chemin DB             : %DATA_DIR%\temperatures.db (defaut)
)
echo.

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

:: --- 4. Enregistrement du chemin d'installation ---
echo [4/6] Enregistrement du chemin d'installation...
echo %INSTALL_DIR%> "%DATA_DIR%\install_path.txt"
echo        Chemin enregistre dans %DATA_DIR%\install_path.txt

:: --- 5. Configuration du chemin DB et copie des scripts ---
echo [5/6] Configuration et copie des scripts de gestion...
if not "!CUSTOM_DB!"=="" (
    powershell -NoProfile -Command ^
        "$p = '!CUSTOM_DB!' -replace '\\', '\\\\'; " ^
        "[System.IO.File]::WriteAllText('%DATA_DIR%\config.toml', \"db_path = \`\"`$p\`\"\`n\")"
    if %errorLevel% neq 0 (
        echo [ERREUR] Impossible d'ecrire config.toml.
        exit /b 1
    )
)
copy /Y "%SCRIPT_DIR%install.bat"   "%DATA_DIR%\install.bat"   >nul
copy /Y "%SCRIPT_DIR%uninstall.bat" "%DATA_DIR%\uninstall.bat" >nul
copy /Y "%SCRIPT_DIR%update.bat"    "%DATA_DIR%\update.bat"    >nul

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
echo    Executable    : %INSTALL_DIR%\%EXE_NAME%
echo    Donnees       : %DATA_DIR%\
echo      config.toml, temperatures.db, install_path.txt
echo      install.bat, uninstall.bat, update.bat
echo.
echo  Verification : sc query MonitoringAlert
echo              ou services.msc

endlocal
