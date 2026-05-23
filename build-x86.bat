@echo off
REM Build doom.exe for Windows x86 (32-bit) with MSVC cl.exe.
REM
REM Run this from an "x86 Native Tools Command Prompt for VS 2022"
REM (Start menu -> "Visual Studio 2022" -> "x86 Native Tools Command Prompt").
REM The compiler in that shell is auto-configured for x86, so /MACHINE:X86
REM below is just an explicit safety belt.

cl /nologo /O2 /W3 /MT doom.c ^
   /link /SUBSYSTEM:WINDOWS /MACHINE:X86 user32.lib gdi32.lib kernel32.lib winmm.lib
if errorlevel 1 exit /b 1
echo Build OK: doom.exe (x86)
