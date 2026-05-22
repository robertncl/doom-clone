@echo off
REM Alternative build using clang (e.g. from MSYS2 CLANGARM64 or LLVM ARM64
REM toolchain). Produces a native ARM64 Windows .exe when run on ARM64.

clang -O2 -Wall -Wno-unused-parameter -o doom.exe doom.c ^
      -luser32 -lgdi32 -lkernel32 -Wl,--subsystem,windows
if errorlevel 1 exit /b 1
echo Build OK: doom.exe
