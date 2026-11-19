param([ValidateSet('build','test','lint','fmt')][string]$Action = 'build')
$ErrorActionPreference = 'Stop'
$clientOldFlags = $env:RUSTFLAGS
$clientOldWrapper = $env:RUSTC_WRAPPER
Push-Location $PSScriptRoot
try {
    $clientCargo = Join-Path $PSScriptRoot '.tools/cargo/bin/cargo.exe'
    if (Test-Path -LiteralPath $clientCargo) {
        $env:RUSTUP_HOME = Join-Path $PSScriptRoot '.tools/rustup'
        $env:CARGO_HOME = Join-Path $PSScriptRoot '.tools/cargo'
    } else { $clientCargo = (Get-Command cargo).Source }
    $clientCompiler = Get-ChildItem '.tools' -Directory -Filter 'llvm-mingw-*-ucrt-x86_64' -ErrorAction SilentlyContinue | Select-Object -First 1
    if ($clientCompiler) {
        $clientBin = Join-Path $clientCompiler.FullName 'bin'
        $env:PATH = $clientBin + ';' + $env:PATH
        $env:CARGO_TARGET_X86_64_PC_WINDOWS_GNU_LINKER = Join-Path $clientBin 'x86_64-w64-mingw32-clang.exe'
        # Host proc macros need the same Unicode-path linker fix as the server.
        $env:RUSTC_WRAPPER = Join-Path $PSScriptRoot 'client/rustc-host.cmd'
    }
    if ($Action -eq 'test') {
        if ($clientCompiler) { $env:RUSTFLAGS = '-C link-self-contained=yes -C link-arg=-fuse-ld=lld' }
        & $clientCargo test --manifest-path client/Cargo.toml
    } elseif ($Action -eq 'fmt') {
        & $clientCargo fmt --manifest-path client/Cargo.toml
    } else {
        $env:RUSTFLAGS = ''
        if ($Action -eq 'lint') {
            & $clientCargo clippy --manifest-path client/Cargo.toml --target wasm32-unknown-unknown -- -D warnings
        } else {
            & $clientCargo build --manifest-path client/Cargo.toml --target wasm32-unknown-unknown --release
            if ($LASTEXITCODE -ne 0) { throw 'No se pudo compilar el cliente Rust.' }
            $clientBindgen = Join-Path $PSScriptRoot '.tools/wasm-bindgen-0.2.130-x86_64-pc-windows-msvc/wasm-bindgen.exe'
            if (-not (Test-Path -LiteralPath $clientBindgen)) { $clientBindgen = (Get-Command wasm-bindgen).Source }
            & $clientBindgen client/target/wasm32-unknown-unknown/release/megalodon_ui.wasm --target web --out-dir static/client --out-name megalodon_ui --no-typescript
        }
    }
    if ($LASTEXITCODE -ne 0) { throw "Cliente: código $LASTEXITCODE" }
} finally { $env:RUSTFLAGS = $clientOldFlags; $env:RUSTC_WRAPPER = $clientOldWrapper; Pop-Location }
