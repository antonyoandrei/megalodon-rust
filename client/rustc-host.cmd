@echo off
set "MEGALODON_WASM_TARGET="
for %%A in (%*) do if "%%~A"=="wasm32-unknown-unknown" set "MEGALODON_WASM_TARGET=1"
if defined MEGALODON_WASM_TARGET (
  %*
) else (
  %* -C link-self-contained=yes -C link-arg=-fuse-ld=lld
)
exit /b %errorlevel%
