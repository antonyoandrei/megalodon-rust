param([ValidateSet('run','build','test','lint','fmt')][string]$Action='run',[switch]$Release,[ValidateSet('megalodon-desktop','megalodon-setup')][string]$Binary='megalodon-desktop',[switch]$CaptureMotion)
$ErrorActionPreference='Stop'
Push-Location $PSScriptRoot
try {
    $desktopCargo=Join-Path $PSScriptRoot '.tools/cargo/bin/cargo.exe'
    if (Test-Path -LiteralPath $desktopCargo) {
        $env:RUSTUP_HOME=Join-Path $PSScriptRoot '.tools/rustup'
        $env:CARGO_HOME=Join-Path $PSScriptRoot '.tools/cargo'
        $desktopCompiler=Get-ChildItem -LiteralPath '.tools' -Directory -Filter 'llvm-mingw-*-ucrt-x86_64' | Select-Object -First 1
        if ($desktopCompiler) {
            $env:PATH=(Join-Path $desktopCompiler.FullName 'bin')+';'+$env:PATH
            $env:CARGO_TARGET_X86_64_PC_WINDOWS_GNU_LINKER=Join-Path $desktopCompiler.FullName 'bin/x86_64-w64-mingw32-clang.exe'
            $env:RUSTFLAGS='-C link-self-contained=yes -C link-arg=-fuse-ld=lld'
        }
    } else { $desktopCargo=(Get-Command cargo -ErrorAction Stop).Source }
    $desktopFeatures = if ($CaptureMotion) { 'desktop,motion-capture' } else { 'desktop' }
    $desktopArgs=@($Action,'--features',$desktopFeatures,'--bin',$Binary)
    if ($Action -eq 'fmt') {$desktopArgs=@('fmt','--all')}
    elseif ($Action -eq 'lint') {$desktopArgs=@('clippy','--features',$desktopFeatures,'--bin',$Binary,'--','-D','warnings')}
    elseif ($Release) {$desktopArgs+='--release'}
    & $desktopCargo @desktopArgs
    if ($LASTEXITCODE -ne 0) {throw "Cargo: $LASTEXITCODE"}
} finally {Pop-Location}
