<#
.SYNOPSIS
    Crée les tâches planifiées MonitoringAlert avec StartWhenAvailable.
    Appelé par install.bat, update.bat et apply-config.bat.

.DESCRIPTION
    Lit la configuration des rapports directement depuis config.toml.
    StartWhenAvailable garantit que si le PC était éteint à l'heure prévue,
    la notification sera envoyée dès la prochaine ouverture de session.
#>
param(
    [Parameter(Mandatory)][string]$ExePath,
    [Parameter(Mandatory)][string]$Username
)

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

# ── Lecture de config.toml ─────────────────────────────────────
$ConfigPath = Join-Path $env:LOCALAPPDATA 'Programs\MonitoringAlert\config.toml'
$Config = Get-Content $ConfigPath -Raw

function Get-TomlBool([string]$Key, [bool]$Default) {
    if ($Config -match "$Key\s*=\s*(\w+)") { return $matches[1] -ieq 'true' }
    return $Default
}
function Get-TomlString([string]$Key, [string]$Default) {
    if ($Config -match "$Key\s*=\s*""([^""]+)""") { return $matches[1] }
    return $Default
}
function Get-TomlInt([string]$Key, [int]$Default) {
    if ($Config -match "$Key\s*=\s*(\d+)") { return [int]$matches[1] }
    return $Default
}

$DailyEnabled   = Get-TomlBool   'daily_report_enabled'   $true
$DailyTime      = Get-TomlString 'daily_report_time'      '08:00'
$WeeklyEnabled  = Get-TomlBool   'weekly_report_enabled'  $false
$WeeklyDay      = Get-TomlString 'weekly_report_day'      'MON'
$WeeklyTime     = Get-TomlString 'weekly_report_time'     '08:00'
$MonthlyEnabled = Get-TomlBool   'monthly_report_enabled' $false
$MonthlyDay     = Get-TomlInt    'monthly_report_day'     1
$MonthlyTime    = Get-TomlString 'monthly_report_time'    '08:00'

# ── Initialisation ─────────────────────────────────────────────
$TaskPath  = '\MonitoringAlert\'
$Settings  = New-ScheduledTaskSettingsSet -StartWhenAvailable
$Principal = New-ScheduledTaskPrincipal -UserId $Username -LogonType Interactive

# ── Suppression des tâches existantes ─────────────────────────
foreach ($name in 'RapportJournalier', 'RapportHebdomadaire', 'RapportMensuel') {
    Unregister-ScheduledTask -TaskPath $TaskPath -TaskName $name `
        -Confirm:$false -ErrorAction SilentlyContinue
}

# ── Rapport journalier ─────────────────────────────────────────
if ($DailyEnabled) {
    $action  = New-ScheduledTaskAction `
        -Execute "powershell.exe" `
        -Argument "-NoProfile -NonInteractive -WindowStyle Hidden -Command `"& '$ExePath' notify --daily`""
    $trigger = New-ScheduledTaskTrigger -Daily -At $DailyTime
    Register-ScheduledTask -TaskPath $TaskPath -TaskName 'RapportJournalier' `
        -Action $action -Trigger $trigger -Settings $Settings -Principal $Principal `
        -Force | Out-Null
    Write-Host "       Rapport journalier    : $DailyTime chaque jour  [StartWhenAvailable]"
} else {
    Write-Host "       Rapport journalier    : desactive"
}

# ── Rapport hebdomadaire ───────────────────────────────────────
if ($WeeklyEnabled) {
    $dayMap = @{
        MON = 'Monday'; TUE = 'Tuesday';  WED = 'Wednesday'; THU = 'Thursday'
        FRI = 'Friday'; SAT = 'Saturday'; SUN = 'Sunday'
    }
    $day = $dayMap[$WeeklyDay.ToUpper()]
    if (-not $day) { $day = 'Monday' }

    $action  = New-ScheduledTaskAction `
        -Execute "powershell.exe" `
        -Argument "-NoProfile -NonInteractive -WindowStyle Hidden -Command `"& '$ExePath' notify --weekly`""
    $trigger = New-ScheduledTaskTrigger -Weekly -DaysOfWeek $day -At $WeeklyTime
    Register-ScheduledTask -TaskPath $TaskPath -TaskName 'RapportHebdomadaire' `
        -Action $action -Trigger $trigger -Settings $Settings -Principal $Principal `
        -Force | Out-Null
    Write-Host "       Rapport hebdomadaire  : $WeeklyDay a $WeeklyTime  [StartWhenAvailable]"
} else {
    Write-Host "       Rapport hebdomadaire  : desactive"
}

# ── Rapport mensuel ────────────────────────────────────────────
# New-ScheduledTaskTrigger n'a pas de paramètre -Monthly.
# On utilise schtasks.exe qui supporte /SC MONTHLY nativement.
if ($MonthlyEnabled) {
    $psArgs = "-NoProfile -NonInteractive -WindowStyle Hidden -Command `"& '$ExePath' notify --monthly`""
    schtasks.exe /Create /F `
        /TN '\MonitoringAlert\RapportMensuel' `
        /TR "powershell.exe $psArgs" `
        /SC MONTHLY /D $MonthlyDay /ST $MonthlyTime `
        /RU $Username | Out-Null
    # Activer StartWhenAvailable via le module ScheduledTasks
    $task = Get-ScheduledTask -TaskPath $TaskPath -TaskName 'RapportMensuel' -ErrorAction SilentlyContinue
    if ($task) {
        $task.Settings.StartWhenAvailable = $true
        Set-ScheduledTask -TaskPath $TaskPath -TaskName 'RapportMensuel' -Settings $task.Settings | Out-Null
    }
    Write-Host "       Rapport mensuel       : jour $MonthlyDay a $MonthlyTime  [StartWhenAvailable]"
} else {
    Write-Host "       Rapport mensuel       : desactive"
}
