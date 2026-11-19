param([switch]$SkipBuild)
$ErrorActionPreference='Stop'
Push-Location $PSScriptRoot
try {
    if (-not (Test-Path -LiteralPath .tools/mpv/libmpv-2.dll)) { & ./scripts/setup-mpv.ps1 }
    & ./scripts/setup-vulkan.ps1
    & ./scripts/setup-rclone.ps1
    if (-not $SkipBuild) {
        & ./desktop.ps1 -Action build -Release
        & ./desktop.ps1 -Action build -Release -Binary megalodon-setup
    }
    & python scripts/licenses.py
    if ($LASTEXITCODE -ne 0) {throw 'No se pudieron recopilar los avisos de dependencias.'}
    & python scripts/package.py
    if ($LASTEXITCODE -ne 0) {throw 'No se pudo crear el paquete.'}
    $packageVersion = [regex]::Match((Get-Content Cargo.toml -Raw), '(?m)^version\s*=\s*"([^"]+)"').Groups[1].Value
    $packageCheck=Start-Process -FilePath (Join-Path $PSScriptRoot "dist/Megalodon-$packageVersion-windows-x64-setup.exe") -ArgumentList '--verify' -WindowStyle Hidden -Wait -PassThru
    if ($packageCheck.ExitCode -ne 0) {throw 'El instalador no ha pasado la verificación de integridad.'}
    Get-ChildItem -LiteralPath dist | Select-Object Name,Length
} finally {Pop-Location}
