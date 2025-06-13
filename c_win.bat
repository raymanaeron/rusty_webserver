@echo off

REM Clean script for Windows
echo Cleaning Rust build artifacts on Windows...

REM Remove target directory and all its contents
if exist "target" (
    echo Removing target directory...
    rmdir /s /q "target"
    echo Target directory removed
) else (
    echo Target directory does not exist
)

echo Cleanup completed!
pause
