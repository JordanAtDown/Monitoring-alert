<#
.SYNOPSIS
    Crée les tâches planifiées MonitoringAlert avec StartWhenAvailable.
    Appelé par install.bat et update.bat.

.DESCRIPTION
    StartWhenAvailable garantit que si le PC était éteint à l'heure prévue,
    la notification sera envoyée dès la prochaine ouverture de session.
#>
param(
    [Parameter(Mandatory)][string]$ExePath,
    [Parameter(Mandatory)][string]$Username,

    [string]$DailyEnabled   = 'false',
    [string]$DailyTime      = '08:00',

    [string]$WeeklyEnabled  = 'false',
    [string]$WeeklyDay      = 'MON',
    [string]$WeeklyTime     = '08:00',

    [string]$MonthlyEnabled = 'false',
    [string]$MonthlyDay     = '1',
    [string]$MonthlyTime    = '08:00'
)

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

$TaskPath  = '\MonitoringAlert\'
$Settings  = New-ScheduledTaskSettingsSet -StartWhenAvailable
$Principal = New-ScheduledTaskPrincipal -UserId $Username -LogonType Interactive

# ── Suppression des tâches existantes ─────────────────────────
foreach ($name in 'RapportJournalier', 'RapportHebdomadaire', 'RapportMensuel') {
    Unregister-ScheduledTask -TaskPath $TaskPath -TaskName $name `
        -Confirm:$false -ErrorAction SilentlyContinue
}

# ── Rapport journalier ─────────────────────────────────────────
if ($DailyEnabled -ieq 'true') {
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
if ($WeeklyEnabled -ieq 'true') {
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
if ($MonthlyEnabled -ieq 'true') {
    $action  = New-ScheduledTaskAction `
        -Execute "powershell.exe" `
        -Argument "-NoProfile -NonInteractive -WindowStyle Hidden -Command `"& '$ExePath' notify --monthly`""
    $trigger = New-ScheduledTaskTrigger -Monthly -DaysOfMonth ([int]$MonthlyDay) -At $MonthlyTime
    Register-ScheduledTask -TaskPath $TaskPath -TaskName 'RapportMensuel' `
        -Action $action -Trigger $trigger -Settings $Settings -Principal $Principal `
        -Force | Out-Null
    Write-Host "       Rapport mensuel       : jour $MonthlyDay a $MonthlyTime  [StartWhenAvailable]"
} else {
    Write-Host "       Rapport mensuel       : desactive"
}
