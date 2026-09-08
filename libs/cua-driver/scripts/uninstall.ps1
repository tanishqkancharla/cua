# Remove only the OpenSky Driver installation.
[CmdletBinding()]
param([switch]$Force, [switch]$ValidateOnly, [switch]$Purge)
& (Join-Path $PSScriptRoot "uninstall-local.ps1") @PSBoundParameters
exit $LASTEXITCODE
