$ErrorActionPreference='Stop'
$vulkanDir=Join-Path (Split-Path -Parent $PSScriptRoot) '.tools/vulkan'
New-Item -ItemType Directory -Path $vulkanDir -Force | Out-Null
$vulkanZip=Join-Path $vulkanDir 'runtime.zip'
if (-not (Test-Path -LiteralPath $vulkanZip)) {
    Invoke-WebRequest -Uri 'https://sdk.lunarg.com/sdk/download/1.4.357.0/windows/VulkanRT-X64-1.4.357.0-Components.zip' -OutFile $vulkanZip
}
if ((Get-FileHash -LiteralPath $vulkanZip -Algorithm SHA256).Hash.ToLowerInvariant() -ne 'a14672efed15aafc7f5a16572d35cd3a3416eadf670aeee3cdf50ee32d5fbf83') {throw 'El runtime Vulkan no coincide con su SHA-256.'}
# Windows bsdtar, not a tar earlier on PATH (Git Bash ships one that misreads C:).
& (Join-Path $env:SystemRoot 'System32/tar.exe') -xf $vulkanZip -C $vulkanDir
if ($LASTEXITCODE -ne 0) {throw 'No se pudo extraer el runtime Vulkan.'}
Write-Output 'Dependencia Vulkan verificada. No se instala ni modifica ningún controlador.'
