@echo off
setlocal EnableDelayedExpansion

:: ============================================================
::  install.bat — Compile, installe et démarre MonitoringAlert
::
::  Usage :
::    install.bat                            DB par défaut (ProgramData)
::    install.bat "D:\Backup\temps.db"       DB personnalisée
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

:: --- 1. Build ---
echo [1/5] Compilation release...
cargo build --release --target %TARGET%
if %errorLevel% neq 0 (
    echo [ERREUR] La compilation a echoue.
    exit /b 1
)

:: --- 2. Copie de l'executable ---
echo [2/5] Installation de l'executable dans "%INSTALL_DIR%"...
if not exist "%INSTALL_DIR%" mkdir "%INSTALL_DIR%"
copy /Y "target\%TARGET%\release\%EXE_NAME%" "%INSTALL_DIR%\%EXE_NAME%" >nul
if %errorLevel% neq 0 (
    echo [ERREUR] Impossible de copier l'executable.
    exit /b 1
)

:: --- 3. Creation du dossier de donnees ---
echo [3/5] Creation du dossier de donnees "%DATA_DIR%"...
if not exist "%DATA_DIR%" mkdir "%DATA_DIR%"

:: --- 4. Configuration du chemin DB ---
if not "%~1"=="" (
    echo [4/5] Ecriture de config.toml avec le chemin DB personnalise...
    powershell -NoProfile -Command ^
        "$p = '%~1' -replace '\\', '\\\\'; " ^
        "[System.IO.File]::WriteAllText('%DATA_DIR%\config.toml', \"db_path = \`\"`$p\`\"\`n\")"
    if %errorLevel% neq 0 (
        echo [ERREUR] Impossible d'ecrire config.toml.
        exit /b 1
    )
    echo        DB : %~1
) else (
    echo [4/5] Aucun chemin DB personnalise -- DB par defaut : %DATA_DIR%\temperatures.db
)

:: --- 5. Enregistrement et demarrage du service ---
echo [5/5] Enregistrement du service Windows...
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
echo Verifiez dans services.msc ou : sc query MonitoringAlert

endlocal
