# doom-clone

A minimal DOOM-style raycasting FPS in a single C file. Same source compiles
for **Windows on ARM64** (Win32 + GDI) and **Linux / WSL on ARM64** (X11) —
the platform layer is selected with `#ifdef _WIN32`.

Verified building and running on aarch64 Ubuntu 24.04 (WSLg) — both
windowed and `--headless` modes exit cleanly.

## Controls

| Key | Action |
| --- | --- |
| `W` / Up arrow | Move forward |
| `S` / Down arrow | Move backward |
| `A` / `D` | Strafe left / right |
| Left / Right arrow | Turn |
| `Space` | Shoot |
| `R` | Restart (after death) |
| `Esc` | Quit |

## Features

- DDA raycasting with distance shading and brick-stripe walls
- Imp-style enemy that chases and melees you
- Pistol with muzzle flash + ammo counter
- HUD (health, ammo), crosshair, death/win banners

## Building

### Windows on ARM64 (MSVC)

1. Install Visual Studio 2022 with **"MSVC v143 — C++ ARM64 build tools"**.
2. Open **"ARM64 Native Tools Command Prompt for VS 2022"** from the Start menu.
3. `cd` into this folder and run:

   ```
   build.bat
   ```

   This produces a native ARM64 `doom.exe`.

Alternative with clang: `build-clang.bat`.

### Linux / WSL (any arch with X11)

```
./build-linux.sh
./doom            # windowed
./doom --headless --frames 120   # smoke test
```

Requires `libx11-dev` (`sudo apt install libx11-dev` on Debian/Ubuntu).

## Files

- `doom.c` — entire game in one translation unit
- `build.bat` — MSVC build for ARM64 Windows
- `build-clang.bat` — clang build for ARM64 Windows
- `build-linux.sh` — gcc build for Linux/WSL

## Architecture notes

- Game logic (raycasting, sprite rendering, HUD, input handling) is fully
  platform-agnostic and writes into a 640×400 32bpp framebuffer.
- The platform layer creates a window, blits the framebuffer, and
  translates raw key events into a small set of game-defined actions
  (`K_FWD`, `K_TURNL`, `K_SHOOT`, …).
- Win32 backend uses `StretchDIBits` for blitting; X11 backend uses
  `XPutImage`. Both upscale the 640×400 frame to a 1280×800 window.
- Timing uses `QueryPerformanceCounter` on Windows and
  `clock_gettime(CLOCK_MONOTONIC)` on POSIX; frame delta is clamped to
  50 ms for stability.
- No architecture-specific intrinsics, so the same source compiles
  unmodified on x64 and ARM64.

## Test runs (verified)

```
# On aarch64 Ubuntu 24.04 (WSLg):
$ ./build-linux.sh
Build OK: ./doom ...
$ ./doom --headless --frames 120 && echo OK
OK
$ ./doom --frames 60      # opens X11 window, runs ~1s, clean exit
```
