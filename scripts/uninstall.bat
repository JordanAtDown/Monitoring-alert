@echo off
setlocal EnableDelayedExpansion

:: ============================================================
::  uninstall.bat — Arrete, desinstalle et nettoie MonitoringAlert
::
::  Lit le dossier d'installation depuis :
::    C:\ProgramData\MonitoringAlert\config.toml  (cle install_dir)
:: ============================================================

:: --- Vérification des droits admin ---
net session >nul 2>&1
if %errorLevel% neq 0 (
    echo [ERREUR] Ce script doit etre execute en tant qu'Administrateur.
    exit /b 1
)

set "DATA_DIR=C:\ProgramData\MonitoringAlert"
set "EXE_NAME=monitoring-alert.exe"
set "CONFIG_FILE=%DATA_DIR%\config.toml"

:: --- Lecture de install_dir depuis config.toml ---
if not exist "%CONFIG_FILE%" (
    echo [AVERTISSEMENT] config.toml introuvable dans %DATA_DIR%
    echo                 Utilisation du chemin par defaut.
    set "INSTALL_DIR=C:\Program Files\MonitoringAlert"
) else (
    for /f "usebackq delims=" %%v in (
        `powershell -NoProfile -Command ^
            "$c = Get-Content '%CONFIG_FILE%' -Raw; " ^
            "if ($c -match 'install_dir\s*=\s*\"([^\"]+)\"') { $Matches[1] }"`
    ) do set "INSTALL_DIR=%%v"
)

if "!INSTALL_DIR!"=="" set "INSTALL_DIR=C:\Program Files\MonitoringAlert"
echo  Dossier d'installation : !INSTALL_DIR!
echo.

:: --- 1. Arrêt du service ---
echo [1/3] Arret du service...
"!INSTALL_DIR!\%EXE_NAME%" service stop >nul 2>&1
timeout /t 3 /nobreak >nul

:: --- 2. Suppression du service et de l'exécutable ---
echo [2/3] Suppression du service Windows...
"!INSTALL_DIR!\%EXE_NAME%" service uninstall
if %errorLevel% neq 0 (
    echo [AVERTISSEMENT] La suppression du service a echoue -- il est peut-etre deja supprime.
)

if exist "!INSTALL_DIR!" (
    rd /s /q "!INSTALL_DIR!"
    echo        Executable supprime : !INSTALL_DIR!
)

:: --- 3. Suppression optionnelle des données ---
echo [3/3] Suppression des donnees...
if exist "%DATA_DIR%" (
    echo.
    echo  Le dossier de donnees contient la base de temperatures et la configuration :
    echo    %DATA_DIR%
    echo.
    set /p "CONFIRM=Supprimer ce dossier ? Les donnees seront perdues. [o/N] : "
    if /i "!CONFIRM!"=="o" (
        rd /s /q "%DATA_DIR%"
        echo        Donnees supprimees.
    ) else (
        echo        Donnees conservees dans %DATA_DIR%
    )
) else (
    echo        Aucun dossier de donnees trouve.
)

echo.
echo Desinstallation terminee.

endlocal
