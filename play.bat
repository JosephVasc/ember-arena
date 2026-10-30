@echo off
rem Double-click to build (if needed) and play Ember Arena.
cd /d "%~dp0"
set "PATH=%USERPROFILE%\.cargo\bin;%PATH%"
cargo run -p game
if errorlevel 1 pause
