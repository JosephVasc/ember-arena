@echo off
rem Double-click to build (if needed) and play Ember Arena.
cd /d "%~dp0"
set "PATH=%USERPROFILE%\.cargoin;%PATH%"
cargo run -p game
if errorlevel 1 pause
