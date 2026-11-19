param(
    [ValidateSet('run', 'test', 'check', 'build', 'fmt', 'lint')]
    [string]$Action = 'run',
    [switch]$Release
)

$ErrorActionPreference = 'Stop'
Push-Location $PSScriptRoot
try {
    # This machine uses a portable compiler. Other machines can use their own Rust.
    $taskCargo = Join-Path $PSScriptRoot '.tools/cargo/bin/cargo.exe'
    if (Test-Path -LiteralPath $taskCargo) {
        $env:RUSTUP_HOME = Join-Path $PSScriptRoot '.tools/rustup'
        $env:CARGO_HOME = Join-Path $PSScriptRoot '.tools/cargo'
        $taskCompiler = Get-ChildItem -LiteralPath (Join-Path $PSScriptRoot '.tools') -Directory -Filter 'llvm-mingw-*-ucrt-x86_64' | Select-Object -First 1
        if ($taskCompiler) {
            $taskBin = Join-Path $taskCompiler.FullName 'bin'
            $env:PATH = $taskBin + ';' + (Join-Path $env:CARGO_HOME 'bin') + ';' + $env:PATH
            $env:CARGO_TARGET_X86_64_PC_WINDOWS_GNU_LINKER = Join-Path $taskBin 'x86_64-w64-mingw32-clang.exe'
            # LLVM's linker supports Windows paths containing accented characters.
            $env:RUSTFLAGS = '-C link-self-contained=yes -C link-arg=-fuse-ld=lld'
        }
    } else {
        $taskCargo = (Get-Command cargo -ErrorAction Stop).Source
    }
    $taskArgs = @($Action)
    if ($Action -eq 'lint') { $taskArgs = @('clippy', '--all-targets', '--', '-D', 'warnings') }
    elseif ($Action -eq 'fmt') { $taskArgs = @('fmt', '--all') }
    elseif ($Release) { $taskArgs += '--release' }
    & $taskCargo @taskArgs
    if ($LASTEXITCODE -ne 0) { throw "Cargo ha terminado con código $LASTEXITCODE." }
} finally {
    Pop-Location
}
