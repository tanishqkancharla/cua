[CmdletBinding()]
param(
    [string]$UninstallerPath = (Join-Path (Split-Path -Parent $PSScriptRoot) "uninstall-local.ps1")
)

Set-StrictMode -Version Latest
$ErrorActionPreference = "Stop"

$tokens = $null
$parseErrors = $null
[void][System.Management.Automation.Language.Parser]::ParseFile(
    $UninstallerPath,
    [ref]$tokens,
    [ref]$parseErrors)
if ($parseErrors.Count -ne 0) {
    throw "uninstall-local.ps1 parse errors: $($parseErrors -join '; ')"
}

$source = Get-Content -LiteralPath $UninstallerPath -Raw
$purgeInvocation = '& $HistoryPurgeHelper history purge-offline --yes'
$runtimeRemoval = 'Remove-Item -LiteralPath $VisibleBinDir'
$purgeIndex = $source.IndexOf($purgeInvocation, [StringComparison]::Ordinal)
$runtimeRemovalIndex = $source.IndexOf($runtimeRemoval, [StringComparison]::Ordinal)

if ($purgeIndex -lt 0) {
    throw "Windows uninstaller does not invoke exact installed-helper history purge"
}
if ($runtimeRemovalIndex -lt 0 -or $purgeIndex -ge $runtimeRemovalIndex) {
    throw "Windows history purge must run before installed runtime removal"
}
if ($source -notmatch 'if \(-not \(Test-Path -LiteralPath \$HistoryPurgeHelper\)\)[\s\S]*?history_purge_incomplete[\s\S]*?exit 1') {
    throw "Windows uninstaller does not fail closed when its exact helper is absent"
}
if ($source -notmatch 'if \(\$historyPurgeExit -ne 0\)[\s\S]*?history_purge_incomplete[\s\S]*?exit 1') {
    throw "Windows uninstaller does not fail closed when native key destruction fails"
}
if ($source -notmatch 'preserved encrypted Computer History') {
    throw "Windows normal-uninstall preservation disclosure is absent"
}
if ($source -notmatch '\$HistoryPurgeHelper = Join-Path \$HomeDir "packages\\current\\opensky-driver.exe"') {
    throw "OpenSky purge must use its exact installed helper"
}
if ($source -notmatch 'if \(\$child.Name -ne "computer-history"\)' -or
    $source -match 'Remove-Item -LiteralPath \$RuntimeDir[^\r\n]*-Recurse') {
    throw "Normal uninstall must preserve Computer History within the runtime directory"
}

$wrapper = Get-Content -LiteralPath (Join-Path (Split-Path -Parent $UninstallerPath) "uninstall.ps1") -Raw
if ($wrapper -notmatch '\[switch\]\$Purge' -or
    $wrapper -notmatch '"uninstall-local.ps1"\) @PSBoundParameters') {
    throw "Public uninstaller must forward explicit purge to the local product uninstaller"
}

Write-Host "Windows Computer History uninstall ordering checks passed."
