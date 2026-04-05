@echo off
setlocal EnableDelayedExpansion

:: ============================================================
::  uninstall.bat — Arrete, desinstalle et nettoie MonitoringAlert
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

:: --- 1. Arrêt du service ---
echo [1/3] Arret du service...
"%INSTALL_DIR%\%EXE_NAME%" service stop >nul 2>&1
:: Attendre que le service s'arrete completement
timeout /t 3 /nobreak >nul

:: --- 2. Désinstallation du service ---
echo [2/3] Suppression du service Windows...
"%INSTALL_DIR%\%EXE_NAME%" service uninstall
if %errorLevel% neq 0 (
    echo [AVERTISSEMENT] La suppression du service a echoue -- il est peut-etre deja supprime.
)

:: Suppression du dossier d'installation
if exist "%INSTALL_DIR%" (
    rd /s /q "%INSTALL_DIR%"
    echo        Executable supprime : %INSTALL_DIR%
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
