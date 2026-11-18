param(
    [Parameter(Mandatory = $true)][string]$Serial,
    [string]$Adb = "$env:LOCALAPPDATA\Android\Sdk\platform-tools\adb.exe"
)

$ErrorActionPreference = 'Stop'

function Invoke-PulsysAdb {
    param([string[]]$Arguments)
    $pulsysOutput = & $Adb -s $Serial @Arguments
    if ($LASTEXITCODE -ne 0) { throw "adb failed: $Arguments" }
    return $pulsysOutput
}

function Get-PulsysForeground {
    $pulsysActivities = Invoke-PulsysAdb @('shell', 'dumpsys', 'activity', 'activities')
    return ($pulsysActivities | Select-String 'mResumedActivity|topResumedActivity') -join "`n"
}

Invoke-PulsysAdb @('shell', 'am', 'force-stop', 'com.pulsys.app') | Out-Null
Invoke-PulsysAdb @('shell', 'am', 'start', '-W', '-n', 'com.pulsys.app/.MainActivity')
$pulsysForeground = Get-PulsysForeground
if ($pulsysForeground -notmatch 'com\.pulsys\.app/') { throw "Pulsys did not open: $pulsysForeground" }

Invoke-PulsysAdb @('shell', 'uiautomator', 'dump', '/sdcard/pulsys-back-test.xml') | Out-Null
[xml]$pulsysUi = (Invoke-PulsysAdb @('exec-out', 'cat', '/sdcard/pulsys-back-test.xml')) -join "`n"
if ($pulsysUi.SelectNodes('//node[@content-desc="Today" and @selected="true"]').Count -ne 1) {
    throw 'Today is not selected before Android Back'
}

Invoke-PulsysAdb @('shell', 'input', 'keyevent', 'KEYCODE_BACK') | Out-Null
$pulsysDeadline = [DateTime]::UtcNow.AddSeconds(10)
do {
    $pulsysForeground = Get-PulsysForeground
    if ($pulsysForeground -notmatch 'com\.pulsys\.app/') { break }
    Start-Sleep -Milliseconds 100
} while ([DateTime]::UtcNow -lt $pulsysDeadline)

if ($pulsysForeground -match 'com\.pulsys\.app/' -or [string]::IsNullOrWhiteSpace($pulsysForeground)) {
    throw "Android Back did not leave Pulsys in the background: $pulsysForeground"
}
Write-Output "PASS $Serial Android Back from selected Today leaves Pulsys: $pulsysForeground"
