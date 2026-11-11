@echo off
setlocal EnableDelayedExpansion

:: ============================================================
::  uninstall.bat — Supprime monitoring-alert (version 1.x OU 2.x)
::
::  N'utilise que des commandes Windows (sc, schtasks, reg, taskkill) : il
::  fonctionne quelle que soit la version installee, et peut etre lance
::  directement depuis l'archive decompressee pour nettoyer une ancienne
::  installation avant de lancer install.bat.
::
::  La base de donnees et config.toml sont CONSERVES, sauf si vous
::  repondez "o" a la question finale.
::
::  A lancer en tant qu'Administrateur (suppression de l'ancien service Windows).
:: ============================================================

net session >nul 2>&1
if %errorLevel% neq 0 (
    echo [ERREUR] Ce script doit etre execute en tant qu'Administrateur.
    echo          Clic droit sur uninstall.bat ^> "Executer en tant qu'administrateur"
    pause
    exit /b 1
)

set "DATA_DIR=%LOCALAPPDATA%\Programs\MonitoringAlert"
set "CONFIG_FILE=%DATA_DIR%\config.toml"
set "INSTALL_DIR="

:: install_dir est lu dans config.toml (absent = meme dossier que config.toml)
if exist "%CONFIG_FILE%" (
    for /f "usebackq delims=" %%v in (`powershell -NoProfile -Command "$q=[char]34;$c=gc '%CONFIG_FILE%' -Raw;if($c-match('(?m)^\s*install_dir\s*=\s*'+$q+'([^'+$q+']+)'+$q)){$matches[1]-replace '\\\\','\'}"`) do set "INSTALL_DIR=%%v"
)
if "!INSTALL_DIR!"=="" set "INSTALL_DIR=%DATA_DIR%"
echo  Executables : !INSTALL_DIR!
echo  Donnees     : %DATA_DIR%
echo.

echo [1/5] Arret des processus...
taskkill /F /IM monitoring-daemon.exe >nul 2>&1
taskkill /F /IM monitoring-alertd.exe >nul 2>&1
sc stop MonitoringAlert >nul 2>&1
timeout /t 5 /nobreak >nul

echo [2/5] Suppression de l'ancien service Windows (1.x)...
sc delete MonitoringAlert >nul 2>&1

echo [3/5] Suppression des taches planifiees de rapport (1.x)...
schtasks /delete /tn "\MonitoringAlert\RapportJournalier" /f >nul 2>&1
schtasks /delete /tn "\MonitoringAlert\RapportHebdomadaire" /f >nul 2>&1
schtasks /delete /tn "\MonitoringAlert\RapportMensuel" /f >nul 2>&1
schtasks /delete /tn "MonitoringAlert" /f >nul 2>&1

echo [4/5] Suppression du demarrage automatique et des notifications (registre)...
reg delete "HKCU\Software\Microsoft\Windows\CurrentVersion\Run" /v "MonitoringAlert" /f >nul 2>&1
reg delete "HKCU\Software\Classes\AppUserModelId\MonitoringAlert.TemperatureMonitor" /f >nul 2>&1

echo [5/5] Suppression des executables et des scripts...
del /f /q "!INSTALL_DIR!\monitoring-alert.exe" "!INSTALL_DIR!\monitoring-daemon.exe" "!INSTALL_DIR!\monitoring-alertd.exe" >nul 2>&1
if /i not "!INSTALL_DIR!"=="%DATA_DIR%" rd "!INSTALL_DIR!" >nul 2>&1
del /f /q "%DATA_DIR%\Register-Tasks.ps1" "%DATA_DIR%\apply-config.bat" "%DATA_DIR%\update.bat" >nul 2>&1

echo.
echo  Le systeme est supprime. Conserves dans "%DATA_DIR%" :
echo    config.toml, la base de donnees (temperatures.db) et les logs.
echo  Ils seront reutilises par install.bat (la base est migree sur place).
echo.
set /p "CONFIRM=Supprimer AUSSI la base de donnees, la configuration et les logs ? Les donnees seront perdues. [o/N] : "
if /i "!CONFIRM!"=="o" (
    rd /s /q "%DATA_DIR%"
    echo  Donnees supprimees.
) else (
    echo  Donnees conservees.
)

echo.
echo Desinstallation terminee.
pause
endlocal
