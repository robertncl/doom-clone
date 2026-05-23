@echo off
REM Build doom.exe with MSVC (cl.exe).
REM
REM Run this from an "ARM64 Native Tools Command Prompt for VS 2022"
REM (Start menu -> "Visual Studio 2022" -> "ARM64 Native Tools Command Prompt").
REM The resulting doom.exe is a native ARM64 Windows binary.
REM
REM From an x64 prompt the same file builds an x64 .exe instead - the
REM source itself is target-agnostic.

cl /nologo /O2 /W3 /MT doom.c ^
   /link /SUBSYSTEM:WINDOWS user32.lib gdi32.lib kernel32.lib winmm.lib
if errorlevel 1 exit /b 1
echo Build OK: doom.exe
