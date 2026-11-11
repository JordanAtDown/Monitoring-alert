@echo off
setlocal

:: ============================================================
::  install.bat — Installe ou met a jour monitoring-alert
::
::  - Installation neuve : copie les fichiers, demarre en arriere-plan,
::    demarrage automatique a l'ouverture de session.
::  - Depuis la version 1.x : supprime automatiquement l'ancien systeme
::    (service Windows, taches de rapport, notifications, scripts) et
::    migre config.toml. La base de donnees est CONSERVEE.
::
::  Lancez-le en tant qu'Administrateur (clic droit) la premiere fois si une
::  version 1.x est installee : supprimer son service Windows l'exige.
::  Sinon, un double-clic suffit.
::
::  Facultatif : editez config.toml (dans ce dossier) avant de lancer ce
::  script pour une installation neuve. Une configuration deja installee
::  n'est jamais ecrasee.
:: ============================================================

if not exist "%~dp0monitoring-alert.exe" (
    echo [ERREUR] monitoring-alert.exe introuvable dans "%~dp0"
    echo          Lancez ce script depuis le dossier de l'archive decompressee.
    pause
    exit /b 1
)

"%~dp0monitoring-alert.exe" install
if errorlevel 1 (
    echo.
    echo [ERREUR] L'installation a echoue. Voir le message ci-dessus.
    pause
    exit /b 1
)

echo.
pause
endlocal
