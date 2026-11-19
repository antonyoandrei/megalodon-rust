param(
    [ValidatePattern('^[A-Za-z0-9_.-]+/[A-Za-z0-9_.-]+$')][string]$Repository,
    [switch]$VerifyOnly
)
$ErrorActionPreference = 'Stop'
[Net.ServicePointManager]::SecurityProtocol = [Net.SecurityProtocolType]::Tls12
$installVersion = [regex]::Match((Get-Content -LiteralPath (Join-Path $PSScriptRoot 'Cargo.toml') -Raw), '(?m)^version\s*=\s*"([^"]+)"').Groups[1].Value
$installName = "Megalodon-$installVersion-windows-x64-setup.exe"
$installFolder = Join-Path $PSScriptRoot 'dist'
$installFile = Join-Path $installFolder $installName
$installChecksums = Join-Path $installFolder 'SHA256SUMS.txt'
$installStamp = Join-Path $installFolder 'build-info.json'
$installCommit = $null
if (Test-Path -LiteralPath (Join-Path $PSScriptRoot '.git')) {
    $installCommit = & git -C $PSScriptRoot rev-parse HEAD
    if ($LASTEXITCODE -ne 0 -or $installCommit -notmatch '^[a-f0-9]{40}$') { throw 'No se pudo identificar el commit que has descargado.' }
}
$installLocalCommit = $null
if (Test-Path -LiteralPath $installStamp) {
    try { $installLocalCommit = (Get-Content -LiteralPath $installStamp -Raw | ConvertFrom-Json).commit } catch { $installLocalCommit = $null }
}
$installNeedsDownload = -not ((Test-Path -LiteralPath $installFile) -and (Test-Path -LiteralPath $installChecksums))
if ($installCommit -and $installCommit -ne $installLocalCommit) { $installNeedsDownload = $true }
if ($installNeedsDownload) {
    if (-not $Repository) {
        $installOrigin = & git -C $PSScriptRoot remote get-url origin 2>$null
        if ($LASTEXITCODE -ne 0 -or $installOrigin -notmatch '^(?:https://github\.com/|git@github\.com:)([A-Za-z0-9_.-]+/[A-Za-z0-9_.-]+?)(?:\.git)?$') {
            throw 'No hay paquete local ni un origen de GitHub. Usa -Repository usuario/repositorio o el instalador de Releases.'
        }
        $Repository = $Matches[1]
    }
    if (-not $installCommit) { throw 'Abre Instalar.cmd desde un clon del repositorio, o descarga el instalador de Releases.' }
    $installTag = "build-$installCommit"
    $installHeaders = @{ Accept='application/vnd.github+json'; 'User-Agent'='Megalodon-Installer' }
    $installToken = $env:GH_TOKEN
    if (-not $installToken -and (Get-Command gh -ErrorAction SilentlyContinue)) {
        $installSavedPreference = $ErrorActionPreference
        try {
            $ErrorActionPreference = 'Continue'
            $installGhToken = & gh auth token 2>$null
            if ($LASTEXITCODE -eq 0) { $installToken = $installGhToken }
        } finally { $ErrorActionPreference = $installSavedPreference }
        Remove-Variable installGhToken -ErrorAction SilentlyContinue
    }
    if ($installToken) { $installHeaders.Authorization = "Bearer $installToken" }
    try {
        $installRelease = Invoke-RestMethod -Uri "https://api.github.com/repos/$Repository/releases/tags/$installTag" -Headers $installHeaders
    } catch {
        throw "Todavía no está disponible el instalador de este commit. Comprueba que el flujo Windows del push a la rama principal haya terminado. En repositorios privados, usa GH_TOKEN o una sesión de gh con acceso a $Repository."
    }
    New-Item -ItemType Directory -Path $installFolder -Force | Out-Null
    foreach ($installAssetName in @($installName, 'SHA256SUMS.txt')) {
        $installAsset = $installRelease.assets | Where-Object name -EQ $installAssetName | Select-Object -First 1
        if (-not $installAsset) { throw "La publicación no contiene $installAssetName." }
        if ($installAsset.url -notmatch '^https://api\.github\.com/repos/') { throw 'El servidor devolvió una dirección de descarga inesperada.' }
        $installDownloadHeaders = $installHeaders.Clone()
        $installDownloadHeaders.Accept = 'application/octet-stream'
        $installDestination = Join-Path $installFolder $installAssetName
        Invoke-WebRequest -UseBasicParsing -Uri $installAsset.url -Headers $installDownloadHeaders -OutFile "$installDestination.download"
        Move-Item -LiteralPath "$installDestination.download" -Destination $installDestination -Force
    }
    Remove-Variable installToken,installHeaders,installDownloadHeaders -ErrorAction SilentlyContinue
}
if (-not (Test-Path -LiteralPath $installChecksums)) { throw 'Falta SHA256SUMS.txt junto al instalador.' }
$installLine = Get-Content -LiteralPath $installChecksums | Where-Object { $_ -match ('^[a-f0-9]{64}  ' + [regex]::Escape($installName) + '$') } | Select-Object -First 1
if (-not $installLine) { throw 'No se encontró la huella del instalador.' }
$installExpected = $installLine.Substring(0,64)
if ((Get-FileHash -LiteralPath $installFile -Algorithm SHA256).Hash.ToLowerInvariant() -ne $installExpected) { throw 'El instalador no supera la verificación SHA-256. No se ejecutará.' }
if ($installCommit) { @{ commit=$installCommit; version=$installVersion } | ConvertTo-Json | Set-Content -LiteralPath $installStamp -Encoding UTF8 }
Write-Output "Megalodon $installVersion verificado."
if (-not $VerifyOnly) {
    $installProcess = Start-Process -FilePath $installFile -WindowStyle Normal -Wait -PassThru
    if ($installProcess.ExitCode -ne 0) { throw 'La instalación no se ha completado.' }
}
