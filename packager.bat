@echo off
echo Building Aegis Http Native Host...
cargo build --release
if %errorlevel% neq 0 (
    echo Build failed!
    exit /b %errorlevel%
)

echo Cleaning up previous packager cache...
if exist "dist\packager\.cargo-packager" (
    rmdir /S /Q "dist\packager\.cargo-packager"
)

echo Packaging Aegis Http Native Host...
cargo packager --release
if %errorlevel% neq 0 (
    echo Packaging failed!
    exit /b %errorlevel%
)

echo.
echo =======================================================
echo Done! The installer has been created successfully.
echo You can find it at: dist\packager\aegis-host_0.1.0_x64-setup.exe
echo =======================================================
pause
