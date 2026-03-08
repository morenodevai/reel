@echo off
echo Building Reel for Windows (vendored deps)...
set PUB_CACHE=%~dp0vendor
flutter build windows --release
if %ERRORLEVEL% EQU 0 (
    echo.
    echo Built: build\windows\x64\runner\Release\reel.exe
) else (
    echo Build failed.
)
