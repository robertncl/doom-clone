@echo off
REM Build doom.exe for Windows x86 (32-bit) with clang.
REM Works from any prompt as long as the LLVM toolchain is on PATH and
REM the i686-pc-windows-msvc target is installed.

clang -O2 -Wall -Wno-unused-parameter -m32 -o doom.exe doom.c ^
      -luser32 -lgdi32 -lkernel32 -lwinmm -Wl,--subsystem,windows
if errorlevel 1 exit /b 1
echo Build OK: doom.exe (x86)
