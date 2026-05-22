# doom-clone

A minimal DOOM-style raycasting FPS for Windows on ARM64 (and x64).

Single-file C, Win32 + GDI only — no SDL, no OpenGL, no external assets.
Builds with either MSVC `cl.exe` or `clang` from the ARM64 toolchain.

![screenshot placeholder]

## Features

- Wolfenstein/Doom-style raycasting walls with distance shading
- Textured "brick" stripes on walls
- One enemy (imp-like silhouette) that chases and attacks
- Pistol with muzzle flash and ammo counter
- Health/ammo HUD, crosshair, death screen with restart

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

## Building on ARM64 Windows

### Option 1 — MSVC (recommended)

1. Install Visual Studio 2022 with the **"MSVC v143 — VS 2022 C++ ARM64 build tools"** workload.
2. Open **"ARM64 Native Tools Command Prompt for VS 2022"** from the Start menu.
3. `cd` to this folder and run:

   ```
   build.bat
   ```

   This produces a native ARM64 `doom.exe`.

### Option 2 — Clang

If you have LLVM/clang for ARM64 Windows (e.g. via MSYS2 `CLANGARM64`):

```
build-clang.bat
```

### Running

Double-click `doom.exe` or run it from a terminal. The window opens at
1280×800 (rendered at 640×400 internally and stretched).

## Files

- `doom.c` — entire game in one translation unit (~500 lines)
- `build.bat` — MSVC build script
- `build-clang.bat` — clang build script

## Architecture notes

- Framebuffer is a single `VirtualAlloc`'d 32bpp buffer, blitted each
  frame via `StretchDIBits`.
- Walls use DDA raycasting; one ray per screen column.
- Floor/ceiling are shaded per-row by perpendicular distance.
- The enemy is a procedurally-drawn billboard with depth tested against
  the per-column wall depth buffer.
- Game timing uses `QueryPerformanceCounter`; frame delta is clamped to
  50ms so the simulation stays stable if the window stalls.

The code is plain C99 with no architecture-specific intrinsics, so the
same source compiles unmodified on x64 and ARM64.
