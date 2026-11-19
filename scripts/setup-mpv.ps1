$ErrorActionPreference='Stop'
$mpvRoot=Join-Path (Split-Path -Parent $PSScriptRoot) '.tools/mpv'
$mpvManifest=Get-Content -LiteralPath (Join-Path $PSScriptRoot 'mpv-build.json') -Raw | ConvertFrom-Json
New-Item -ItemType Directory -Path $mpvRoot -Force | Out-Null
$mpvArchive=Join-Path $mpvRoot 'package.7z'
if (-not (Test-Path -LiteralPath $mpvArchive)) {Invoke-WebRequest -Uri $mpvManifest.browser_download_url -OutFile $mpvArchive}
$mpvDigest=(Get-FileHash -LiteralPath $mpvArchive -Algorithm SHA256).Hash.ToLowerInvariant()
if ($mpvDigest -ne $mpvManifest.digest.Replace('sha256:','')) {throw 'El archivo de libmpv no coincide con su SHA-256.'}
# Windows bsdtar, not a tar earlier on PATH (Git Bash ships one that misreads C:).
& (Join-Path $env:SystemRoot 'System32/tar.exe') -xf $mpvArchive -C $mpvRoot
if ($LASTEXITCODE -ne 0) {throw 'No se pudo extraer libmpv.'}
Copy-Item -LiteralPath (Join-Path $PSScriptRoot 'mpv-build.json') -Destination (Join-Path $mpvRoot 'source.json')
Write-Output 'Motor multimedia verificado y preparado.'
