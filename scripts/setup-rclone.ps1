$ErrorActionPreference='Stop'
$rcloneRoot=Join-Path (Split-Path $PSScriptRoot -Parent) '.tools/rclone'
$rcloneArchive=Join-Path $rcloneRoot 'rclone-v1.76.0-windows-amd64.zip'
$rcloneHash='8b4fd2737ade4e70c057ef4a49deb7cffe94b0fb9d7587c189d149823f21a1dc'
New-Item -ItemType Directory -Force -Path $rcloneRoot | Out-Null
if (-not (Test-Path -LiteralPath $rcloneArchive)) {
    Invoke-WebRequest -Uri 'https://github.com/rclone/rclone/releases/download/v1.76.0/rclone-v1.76.0-windows-amd64.zip' -OutFile $rcloneArchive
}
if ((Get-FileHash -LiteralPath $rcloneArchive -Algorithm SHA256).Hash.ToLowerInvariant() -ne $rcloneHash) {throw 'La descarga de rclone no supera la comprobación SHA-256.'}
Expand-Archive -LiteralPath $rcloneArchive -DestinationPath $rcloneRoot -Force
Copy-Item -LiteralPath (Join-Path $rcloneRoot 'rclone-v1.76.0-windows-amd64/rclone.exe') -Destination (Join-Path $rcloneRoot 'rclone.exe') -Force
Write-Output 'rclone preparado y verificado.'
