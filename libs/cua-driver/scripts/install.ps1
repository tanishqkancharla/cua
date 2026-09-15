# Build OpenSky Driver from this checkout; no upstream release fallback.
[CmdletBinding()]
param([switch]$AutoStart = $true, [switch]$NoAutoStart, [switch]$NoPathUpdate)
& (Join-Path $PSScriptRoot "install-local.ps1") @PSBoundParameters
exit $LASTEXITCODE
