@echo off
setlocal enabledelayedexpansion

echo ===================================================
echo   S3D Core - WebAssembly Showcase Demo Runner
echo ===================================================

:: Ensure rustup toolchain and cargo bin are in PATH ahead of any standalone rust
if exist "%USERPROFILE%\.rustup\toolchains\stable-x86_64-pc-windows-msvc\bin" (
    set "PATH=%USERPROFILE%\.rustup\toolchains\stable-x86_64-pc-windows-msvc\bin;%PATH%"
)
if exist "%USERPROFILE%\.cargo\bin" (
    set "PATH=%USERPROFILE%\.cargo\bin;%PATH%"
)

:: Check for trunk
where trunk >nul 2>&1
if %ERRORLEVEL% neq 0 (
    echo [ERROR] 'trunk' was not found in PATH or ~/.cargo/bin.
    echo Please install trunk using: cargo install --locked trunk
    pause
    exit /b 1
)

:: Ensure wasm32-unknown-unknown target is installed
rustup target list --installed | findstr /i "wasm32-unknown-unknown" >nul 2>&1
if %ERRORLEVEL% neq 0 (
    echo [INFO] Installing wasm32-unknown-unknown target...
    rustup target add wasm32-unknown-unknown
    if %ERRORLEVEL% neq 0 (
        echo [ERROR] Failed to install wasm32-unknown-unknown target.
        pause
        exit /b 1
    )
)

echo.
echo [INFO] Starting Trunk server at http://127.0.0.1:8080 ...
echo [INFO] Opening default web browser...
echo.

trunk serve demo.html --example showcase --open %*

if %ERRORLEVEL% neq 0 (
    echo.
    echo [ERROR] Trunk exited with error code %ERRORLEVEL%.
    pause
)
