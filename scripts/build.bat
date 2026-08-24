@echo off
rem Build helper: loads the MSVC x64 dev environment (provides link.exe, rc.exe,
rem INCLUDE/LIB) so that Git Bash's GNU /usr/bin/link.exe does NOT shadow the
rem MSVC linker. Usage:  scripts\build.bat [cargo args...]
set "VSDIR=C:\Program Files\Microsoft Visual Studio\18\Community"
call "%VSDIR%\VC\Auxiliary\Build\vcvarsall.bat" x64 >nul
if errorlevel 1 exit /b 1
cargo %*
exit /b %errorlevel%
