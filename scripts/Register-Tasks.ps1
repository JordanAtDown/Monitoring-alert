<#
.SYNOPSIS
    monitoring-alert-migration-shim : pont de migration depuis la version 1.x.

.DESCRIPTION
    Dans la version 1.x, ce script creait les taches planifiees de rapport et etait
    appele par update.bat. Il n'y a plus ni rapports ni taches : ce fichier ne sert
    plus qu'a faire aboutir une mise a jour lancee par l'ANCIEN update.bat, qui
    remplace monitoring-alert.exe puis execute ce script avec -ExePath et -Username.

    Il lance l'installation de la nouvelle version, qui supprime l'ancien service
    Windows, les anciennes taches planifiees et le reste, migre config.toml et
    demarre le nouveau systeme. La base de donnees est conservee.

    Ne pas supprimer : la mise a jour d'une installation 1.x en depend.
    Le marqueur "monitoring-alert-migration-shim" ci-dessus permet a l'installation
    de le distinguer de l'ancien script.
#>
param(
    [string]$ExePath,
    [string]$Username
)

$ErrorActionPreference = 'Stop'

if (-not $ExePath -or -not (Test-Path -LiteralPath $ExePath)) {
    Write-Host "       [ERREUR] Executable introuvable : '$ExePath'"
    exit 1
}

Write-Host "       Migration vers la version 2 (plus de service Windows, plus de rapports)..."
& $ExePath install
exit $LASTEXITCODE
