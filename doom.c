/*
 * doom.c - Enhanced DOOM-style raycasting FPS.
 *
 * Features:
 *   - Textured walls (stone, brick, metal, wood) - procedural textures
 *   - Textured floor and ceiling via floorcasting
 *   - Multiple enemy types: grunts (melee) and imps (fireballs)
 *   - Blood/spark particle effects
 *   - Health and ammo pickups
 *   - Three levels with auto-progression
 *   - Detailed HUD with face indicator
 *
 * Targets ARM64 Windows (Win32 + GDI) and any POSIX system with X11.
 * Single translation unit, no external deps beyond OS SDK / libX11.
 *
 * Controls:
 *   W / Up       - move forward
 *   S / Down     - move backward
 *   A / D        - strafe left / right
 *   Left / Right - turn
 *   Space        - shoot
 *   R            - restart after death
 *   Esc          - quit
 */

#include <math.h>
#include <stdint.h>
#include <stdio.h>
#include <string.h>
#include <stdlib.h>

#ifdef _WIN32
  #define WIN32_LEAN_AND_MEAN
  #include <windows.h>
#else
  #include <X11/Xlib.h>
  #include <X11/Xutil.h>
  #include <X11/keysym.h>
  #include <time.h>
  #include <unistd.h>
#endif

#ifndef M_PI
#define M_PI 3.14159265358979323846
#endif

#define SCREEN_W       640
#define SCREEN_H       400
#define MAP_W          16
#define MAP_H          16
#define FOV            (M_PI / 3.0)
#define MAX_DEPTH      24.0
#define TEX_SIZE       64
#define LEVEL_COUNT    3
#define MAX_ENEMIES    16
#define MAX_PARTICLES  192
#define MAX_FIREBALLS  16
#define MAX_PICKUPS    16

enum {
    K_FWD, K_BACK, K_STRAFEL, K_STRAFER,
    K_TURNL, K_TURNR,
    K_SHOOT, K_RESTART, K_QUIT,
    K_COUNT
};

enum {
    WALL_NONE = 0,
    WALL_STONE,
    WALL_BRICK,
    WALL_METAL,
    WALL_WOOD,
    WALL_KIND_MAX
};

enum { EN_GRUNT = 0, EN_IMP = 1 };
enum { PU_HEALTH = 0, PU_AMMO = 1 };

static int g_keys[K_COUNT];
static int g_keyEdge[K_COUNT];
static int g_running = 1;
static int g_muzzleFlash = 0;
static int g_level = 0;
static int g_showIntro = 1;
static double g_levelClearTimer = 0;
static double g_painFlash = 0;
static double g_globalTime = 0;

typedef struct {
    double x, y, angle;
    int health;
    int armor;
    int ammo;
} Player;

typedef struct {
    double x, y;
    int type;
    int alive;
    int hp;
    double hitFlash;
    double atkCool;
    double anim;
} Enemy;

typedef struct {
    double x, y, vx, vy;
    int alive;
    double life;
} Fireball;

typedef struct {
    double x, y, vx, vy;
    double life;
    uint32_t color;
} Particle;

typedef struct {
    double x, y;
    int alive;
    int type;
} Pickup;

static Player    g_player;
static Enemy     g_enemies[MAX_ENEMIES];
static Fireball  g_fireballs[MAX_FIREBALLS];
static Particle  g_parts[MAX_PARTICLES];
static Pickup    g_pickups[MAX_PICKUPS];

static uint32_t *g_pixels;
static double    g_depth[SCREEN_W];

static uint32_t g_wallTex[WALL_KIND_MAX][TEX_SIZE * TEX_SIZE];
static uint32_t g_floorTex[TEX_SIZE * TEX_SIZE];
static uint32_t g_ceilTex[TEX_SIZE * TEX_SIZE];

/* Map encoding:
 *   '#'  stone wall
 *   '='  brick wall
 *   'B'  metal wall
 *   'D'  wood (door-look)
 *   '.'  floor
 *   'p'  player spawn
 *   'g'  grunt spawn
 *   'i'  imp spawn
 *   'h'  health pickup
 *   'a'  ammo pickup
 */
static const char *g_levels[LEVEL_COUNT][MAP_H] = {
    {
        "################",
        "#p.............#",
        "#..==..====....#",
        "#..=...g...=.a.#",
        "#..=.......=...#",
        "#..====....=...#",
        "#...h..........#",
        "#......=====...#",
        "#......=.g.....#",
        "#......=...==..#",
        "#......=.......#",
        "#......========#",
        "#..i...........#",
        "#..===......h..#",
        "#.........g....#",
        "################",
    },
    {
        "BBBBBBBBBBBBBBBB",
        "Bp..B....B.....B",
        "B...B....B.g...B",
        "B...D.h..D.....B",
        "B...B....BBBB..B",
        "B...B....i.....B",
        "B...BBBBBB.....B",
        "B.g......B...a.B",
        "B........B..BBBB",
        "BBBB.h...B.....B",
        "B........B.g...B",
        "B...BBBBBB.....B",
        "B...B......i...B",
        "B...B....BBBBBBB",
        "B.a.B......g...B",
        "BBBBBBBBBBBBBBBB",
    },
    {
        "################",
        "#p.#.....g.....#",
        "#..#.########..#",
        "#....#..h..=.a.#",
        "####.#.====.=..#",
        "#.a..#.=i=..=..#",
        "#.####.=.=..=..#",
        "#....g.=.=..=..#",
        "#.######.=..=..#",
        "#.h......=..=..#",
        "#.########..=..#",
        "#......i....=..#",
        "#.##########=..#",
        "#......a....=..#",
        "#.============g#",
        "################",
    },
};

static char g_curMap[MAP_H][MAP_W + 1];

/* ========================================================================
 * Color and noise helpers
 * ======================================================================== */

static uint32_t makeColor(int r, int g, int b)
{
    if (r < 0) r = 0;
    if (r > 255) r = 255;
    if (g < 0) g = 0;
    if (g > 255) g = 255;
    if (b < 0) b = 0;
    if (b > 255) b = 255;
    return (uint32_t)((r << 16) | (g << 8) | b);
}

static int hash2(int x, int y)
{
    unsigned h = (unsigned)(x * 374761393u + y * 668265263u);
    h = (h ^ (h >> 13)) * 1274126177u;
    h ^= (h >> 16);
    return (int)(h & 0xFF);
}

static uint32_t shadeColor(uint32_t c, double mul)
{
    if (mul < 0) mul = 0;
    int r = (int)(((c >> 16) & 0xFF) * mul);
    int g = (int)(((c >> 8)  & 0xFF) * mul);
    int b = (int)(( c        & 0xFF) * mul);
    return makeColor(r, g, b);
}

/* ========================================================================
 * Procedural texture build
 * ======================================================================== */

static void buildTextures(void)
{
    for (int v = 0; v < TEX_SIZE; v++) {
        for (int u = 0; u < TEX_SIZE; u++) {
            /* Stone */
            int n = hash2(u, v) - 128;
            int crack = ((u + v * 2) % 19 == 0 || (u * 2 - v + 64) % 23 == 0) ? -50 : 0;
            int sR = 130 + n / 4 + crack;
            int sG = 125 + n / 4 + crack;
            int sB = 115 + n / 4 + crack;
            g_wallTex[WALL_STONE][v * TEX_SIZE + u] = makeColor(sR, sG, sB);

            /* Brick: alternating-row offset, mortar lines */
            int row = v / 8;
            int colOff = (row & 1) ? 8 : 0;
            int brickU = (u + colOff) % 16;
            int brickV = v % 8;
            int mortar = (brickV == 0 || brickV == 7 ||
                          brickU == 0 || brickU == 15);
            int bn = hash2(u / 2, v / 2) - 128;
            uint32_t brickCol;
            if (mortar) {
                brickCol = makeColor(70, 60, 55);
            } else {
                brickCol = makeColor(150 + bn / 4, 65 + bn / 8, 50 + bn / 8);
            }
            g_wallTex[WALL_BRICK][v * TEX_SIZE + u] = brickCol;

            /* Metal: 32x32 panels with rivets and bevel */
            int pU = u % 32, pV = v % 32;
            int bevel = (pU < 2 || pU > 29 || pV < 2 || pV > 29);
            int hi = (pU < 1 || pV < 1);
            int rivet = ((pU == 5 || pU == 26) && (pV == 5 || pV == 26));
            int mn = hash2(u, v) - 128;
            uint32_t metalCol;
            if (rivet) metalCol = makeColor(210, 210, 220);
            else if (hi) metalCol = makeColor(150, 160, 190);
            else if (bevel) metalCol = makeColor(40, 50, 70);
            else metalCol = makeColor(80 + mn / 6, 95 + mn / 6, 130 + mn / 6);
            g_wallTex[WALL_METAL][v * TEX_SIZE + u] = metalCol;

            /* Wood: grain stripes with knots */
            int wn = hash2(u / 4, v) - 128;
            int grain = (int)(18.0 * sin(u * 0.42 + wn * 0.05));
            int band = (v % 22 < 2) ? -35 : 0;
            int knot = ((u - 22) * (u - 22) + (v - 30) * (v - 30) < 10) ? -45 : 0;
            int wR = 115 + grain + wn / 8 + band + knot;
            int wG = 70  + grain / 2 + wn / 10 + band + knot;
            int wB = 30  + wn / 12 + band + knot;
            g_wallTex[WALL_WOOD][v * TEX_SIZE + u] = makeColor(wR, wG, wB);

            /* Floor tile */
            int tU = u % 16, tV = v % 16;
            int grout = (tU == 0 || tV == 0 || tU == 15 || tV == 15);
            int fn = hash2(u, v) - 128;
            uint32_t floorCol;
            if (grout) floorCol = makeColor(25, 25, 30);
            else floorCol = makeColor(75 + fn / 6, 70 + fn / 6, 60 + fn / 8);
            g_floorTex[v * TEX_SIZE + u] = floorCol;

            /* Ceiling dark */
            int cn = hash2(u + 17, v + 31) - 128;
            g_ceilTex[v * TEX_SIZE + u] =
                makeColor(38 + cn / 8, 36 + cn / 8, 44 + cn / 8);
        }
    }
}

/* ========================================================================
 * Map access and level loading
 * ======================================================================== */

static int mapWallType(int mx, int my)
{
    if (mx < 0 || mx >= MAP_W || my < 0 || my >= MAP_H) return WALL_STONE;
    char c = g_curMap[my][mx];
    switch (c) {
    case '#': return WALL_STONE;
    case '=': return WALL_BRICK;
    case 'B': return WALL_METAL;
    case 'D': return WALL_WOOD;
    default:  return WALL_NONE;
    }
}

static int mapBlocked(int mx, int my)
{
    return mapWallType(mx, my) != WALL_NONE;
}

static void resetTransients(void)
{
    memset(g_enemies,   0, sizeof(g_enemies));
    memset(g_fireballs, 0, sizeof(g_fireballs));
    memset(g_parts,     0, sizeof(g_parts));
    memset(g_pickups,   0, sizeof(g_pickups));
}

static void loadLevel(int n)
{
    resetTransients();
    int eIdx = 0, pIdx = 0;
    for (int y = 0; y < MAP_H; y++) {
        for (int x = 0; x < MAP_W; x++) {
            char c = g_levels[n][y][x];
            char dest = c;
            switch (c) {
            case 'p':
                g_player.x = x + 0.5;
                g_player.y = y + 0.5;
                g_player.angle = 0.0;
                dest = '.';
                break;
            case 'g':
                if (eIdx < MAX_ENEMIES) {
                    g_enemies[eIdx].x = x + 0.5;
                    g_enemies[eIdx].y = y + 0.5;
                    g_enemies[eIdx].type = EN_GRUNT;
                    g_enemies[eIdx].alive = 1;
                    g_enemies[eIdx].hp = 2;
                    g_enemies[eIdx].anim = (x + y) * 0.7;
                    eIdx++;
                }
                dest = '.';
                break;
            case 'i':
                if (eIdx < MAX_ENEMIES) {
                    g_enemies[eIdx].x = x + 0.5;
                    g_enemies[eIdx].y = y + 0.5;
                    g_enemies[eIdx].type = EN_IMP;
                    g_enemies[eIdx].alive = 1;
                    g_enemies[eIdx].hp = 3;
                    g_enemies[eIdx].anim = (x + y) * 0.5;
                    eIdx++;
                }
                dest = '.';
                break;
            case 'h':
                if (pIdx < MAX_PICKUPS) {
                    g_pickups[pIdx].x = x + 0.5;
                    g_pickups[pIdx].y = y + 0.5;
                    g_pickups[pIdx].type = PU_HEALTH;
                    g_pickups[pIdx].alive = 1;
                    pIdx++;
                }
                dest = '.';
                break;
            case 'a':
                if (pIdx < MAX_PICKUPS) {
                    g_pickups[pIdx].x = x + 0.5;
                    g_pickups[pIdx].y = y + 0.5;
                    g_pickups[pIdx].type = PU_AMMO;
                    g_pickups[pIdx].alive = 1;
                    pIdx++;
                }
                dest = '.';
                break;
            }
            g_curMap[y][x] = dest;
        }
        g_curMap[y][MAP_W] = 0;
    }
    g_level = n;
    g_levelClearTimer = 0;
}

/* ========================================================================
 * Pixel/Rect helpers
 * ======================================================================== */

static void putPixel(int x, int y, uint32_t c)
{
    if ((unsigned)x >= SCREEN_W || (unsigned)y >= SCREEN_H) return;
    g_pixels[y * SCREEN_W + x] = c;
}

static void fillRect(int x0, int y0, int w, int h, uint32_t c)
{
    int x1 = x0 + w, y1 = y0 + h;
    if (x0 < 0) x0 = 0;
    if (y0 < 0) y0 = 0;
    if (x1 > SCREEN_W) x1 = SCREEN_W;
    if (y1 > SCREEN_H) y1 = SCREEN_H;
    for (int y = y0; y < y1; y++)
        for (int x = x0; x < x1; x++)
            g_pixels[y * SCREEN_W + x] = c;
}

/* ========================================================================
 * Raycast: textured walls, textured floor + ceiling
 * ======================================================================== */

static void castColumn(int col)
{
    double cameraX = 2.0 * col / (double)SCREEN_W - 1.0;
    double dirX = cos(g_player.angle);
    double dirY = sin(g_player.angle);
    double planeX = -sin(g_player.angle) * tan(FOV / 2.0);
    double planeY =  cos(g_player.angle) * tan(FOV / 2.0);

    double rayDirX = dirX + planeX * cameraX;
    double rayDirY = dirY + planeY * cameraX;

    int mapX = (int)g_player.x;
    int mapY = (int)g_player.y;

    double deltaX = (rayDirX == 0) ? 1e30 : fabs(1.0 / rayDirX);
    double deltaY = (rayDirY == 0) ? 1e30 : fabs(1.0 / rayDirY);

    int stepX, stepY;
    double sideX, sideY;

    if (rayDirX < 0) { stepX = -1; sideX = (g_player.x - mapX) * deltaX; }
    else             { stepX =  1; sideX = (mapX + 1.0 - g_player.x) * deltaX; }
    if (rayDirY < 0) { stepY = -1; sideY = (g_player.y - mapY) * deltaY; }
    else             { stepY =  1; sideY = (mapY + 1.0 - g_player.y) * deltaY; }

    int hit = 0, side = 0, iter = 0, wallType = WALL_STONE;
    while (!hit && iter++ < 128) {
        if (sideX < sideY) { sideX += deltaX; mapX += stepX; side = 0; }
        else               { sideY += deltaY; mapY += stepY; side = 1; }
        int t = mapWallType(mapX, mapY);
        if (t) { hit = 1; wallType = t; }
    }

    double perpDist = (side == 0) ? (sideX - deltaX) : (sideY - deltaY);
    if (perpDist < 0.0001) perpDist = 0.0001;
    g_depth[col] = perpDist;

    int lineH = (int)(SCREEN_H / perpDist);
    int drawStart = -lineH / 2 + SCREEN_H / 2;
    int drawEnd   =  lineH / 2 + SCREEN_H / 2;
    int clipStart = drawStart < 0 ? 0 : drawStart;
    int clipEnd   = drawEnd >= SCREEN_H ? SCREEN_H - 1 : drawEnd;

    /* Wall U coord at hit */
    double wallHitX;
    if (side == 0) wallHitX = g_player.y + perpDist * rayDirY;
    else           wallHitX = g_player.x + perpDist * rayDirX;
    wallHitX -= floor(wallHitX);

    int texU = (int)(wallHitX * TEX_SIZE);
    if (side == 0 && rayDirX > 0) texU = TEX_SIZE - texU - 1;
    if (side == 1 && rayDirY < 0) texU = TEX_SIZE - texU - 1;
    if (texU < 0) texU = 0;
    if (texU >= TEX_SIZE) texU = TEX_SIZE - 1;

    double shade = 1.0 - perpDist / MAX_DEPTH;
    if (shade < 0.18) shade = 0.18;
    if (side == 1) shade *= 0.72;

    double step = (double)TEX_SIZE / lineH;
    double texPos = (clipStart - SCREEN_H / 2 + lineH / 2) * step;
    for (int y = clipStart; y <= clipEnd; y++) {
        int texV = ((int)texPos) & (TEX_SIZE - 1);
        texPos += step;
        uint32_t c = g_wallTex[wallType][texV * TEX_SIZE + texU];
        g_pixels[y * SCREEN_W + col] = shadeColor(c, shade);
    }

    /* Floor/ceiling cast for rows below the wall (and mirror to above) */
    int floorStart = drawEnd + 1;
    if (floorStart <= SCREEN_H / 2) floorStart = SCREEN_H / 2 + 1;
    if (floorStart < 0) floorStart = 0;
    for (int y = floorStart; y < SCREEN_H; y++) {
        double p = y - SCREEN_H / 2.0;
        if (p <= 0) continue;
        double rowDist = (SCREEN_H * 0.5) / p;
        double floorX = g_player.x + rowDist * rayDirX;
        double floorY = g_player.y + rowDist * rayDirY;
        int fu = ((int)(floorX * TEX_SIZE)) & (TEX_SIZE - 1);
        int fv = ((int)(floorY * TEX_SIZE)) & (TEX_SIZE - 1);
        if (fu < 0) fu += TEX_SIZE;
        if (fv < 0) fv += TEX_SIZE;
        double fb = 1.0 - rowDist / MAX_DEPTH;
        if (fb < 0.1) fb = 0.1;
        g_pixels[y * SCREEN_W + col] =
            shadeColor(g_floorTex[fv * TEX_SIZE + fu], fb);
        int cy = SCREEN_H - y - 1;
        if (cy >= 0 && cy < drawStart) {
            g_pixels[cy * SCREEN_W + col] =
                shadeColor(g_ceilTex[fv * TEX_SIZE + fu], fb * 0.85);
        }
    }
}

/* ========================================================================
 * Procedural enemy / pickup / fireball sprite drawing
 * ======================================================================== */

static int gruntPixel(double u, double v, uint32_t *out, double anim)
{
    double cx = u - 0.5;
    double cy = v - 0.5;
    double sway = sin(anim) * 0.015;
    cx -= sway;

    /* helmet dome */
    if (cx * cx + (cy + 0.36) * (cy + 0.36) < 0.022 && cy < -0.30) {
        *out = 0x405028; return 1;
    }
    /* helmet rim */
    if (fabs(cx) < 0.15 && cy > -0.32 && cy < -0.27) {
        *out = 0x202810; return 1;
    }
    /* face skin */
    if (cx * cx + (cy + 0.22) * (cy + 0.22) < 0.016) {
        *out = 0xC0A080; return 1;
    }
    /* eyes */
    if ((cx - 0.05) * (cx - 0.05) + (cy + 0.22) * (cy + 0.22) < 0.0014) {
        *out = 0x101010; return 1;
    }
    if ((cx + 0.05) * (cx + 0.05) + (cy + 0.22) * (cy + 0.22) < 0.0014) {
        *out = 0x101010; return 1;
    }
    /* mouth */
    if (fabs(cx) < 0.04 && cy > -0.14 && cy < -0.11) {
        *out = 0x401010; return 1;
    }
    /* shoulders */
    if (cy > -0.10 && cy < -0.04 && fabs(cx) < 0.25) {
        *out = 0x3C5028; return 1;
    }
    /* torso (vest) */
    if (cy > -0.04 && cy < 0.20 && fabs(cx) < 0.20) {
        *out = 0x506838; return 1;
    }
    /* vest stripes */
    if (cy > -0.02 && cy < 0.02 && fabs(cx) < 0.18) {
        *out = 0x304018; return 1;
    }
    /* belt */
    if (cy > 0.20 && cy < 0.24 && fabs(cx) < 0.22) {
        *out = 0x181208; return 1;
    }
    /* belt buckle */
    if (cy > 0.20 && cy < 0.25 && fabs(cx) < 0.04) {
        *out = 0xC0A030; return 1;
    }
    /* legs */
    if (cy > 0.24 && cy < 0.46) {
        if (cx > -0.18 && cx < -0.03) { *out = 0x2C3818; return 1; }
        if (cx > 0.03 && cx < 0.18)   { *out = 0x2C3818; return 1; }
    }
    /* boots */
    if (cy > 0.42 && cy < 0.50 && (fabs(cx + 0.105) < 0.085 ||
                                    fabs(cx - 0.105) < 0.085)) {
        *out = 0x100804; return 1;
    }
    /* gun in right hand */
    if (cy > 0.02 && cy < 0.06 && cx > 0.20 && cx < 0.34) {
        *out = 0x282828; return 1;
    }
    if (cy > 0.06 && cy < 0.10 && cx > 0.22 && cx < 0.30) {
        *out = 0x181818; return 1;
    }
    return 0;
}

static int impPixel(double u, double v, uint32_t *out, double anim)
{
    double cx = u - 0.5;
    double cy = v - 0.5;
    double bob = sin(anim * 2.0) * 0.02;
    cy -= bob;

    /* horns (left + right tipped triangles) */
    if (cy > -0.50 && cy < -0.34) {
        double t = (cy + 0.50) / 0.16;
        double halfW = 0.05 * t;
        if (cx > -0.18 - halfW && cx < -0.18 + halfW) {
            *out = (cy < -0.46) ? 0x101010 : 0x402010; return 1;
        }
        if (cx >  0.18 - halfW && cx <  0.18 + halfW) {
            *out = (cy < -0.46) ? 0x101010 : 0x402010; return 1;
        }
    }
    /* head */
    if (cx * cx * 1.0 + (cy + 0.30) * (cy + 0.30) * 1.2 < 0.028) {
        *out = 0x803020; return 1;
    }
    /* skull ridges */
    if (cy > -0.42 && cy < -0.38 && fabs(cx) < 0.12) {
        *out = 0x602010; return 1;
    }
    /* glowing eyes */
    if ((cx - 0.07) * (cx - 0.07) + (cy + 0.28) * (cy + 0.28) < 0.0022) {
        *out = 0xFFE020; return 1;
    }
    if ((cx + 0.07) * (cx + 0.07) + (cy + 0.28) * (cy + 0.28) < 0.0022) {
        *out = 0xFFE020; return 1;
    }
    /* nose */
    if (fabs(cx) < 0.012 && cy > -0.24 && cy < -0.20) {
        *out = 0x501808; return 1;
    }
    /* mouth gash */
    if (cy > -0.18 && cy < -0.15 && fabs(cx) < 0.08) {
        *out = 0x200804; return 1;
    }
    /* fangs */
    if (cy > -0.15 && cy < -0.11 && (fabs(cx + 0.04) < 0.012 ||
                                      fabs(cx - 0.04) < 0.012)) {
        *out = 0xF0E8C0; return 1;
    }
    /* body */
    if (cx * cx * 1.6 + (cy - 0.06) * (cy - 0.06) * 0.9 < 0.06) {
        *out = 0x6A1810; return 1;
    }
    /* body spots */
    if ((cx + 0.10) * (cx + 0.10) + (cy + 0.02) * (cy + 0.02) < 0.0035) {
        *out = 0x401008; return 1;
    }
    if ((cx - 0.08) * (cx - 0.08) + (cy - 0.10) * (cy - 0.10) < 0.0035) {
        *out = 0x401008; return 1;
    }
    /* arms with claws */
    double armSwing = sin(anim * 2.0) * 0.04;
    if (cy > -0.05 + armSwing && cy < 0.22 + armSwing) {
        if (cx > 0.18 && cx < 0.32) { *out = 0x501008; return 1; }
        if (cx > -0.32 && cx < -0.18) { *out = 0x501008; return 1; }
    }
    /* claws */
    if (cy > 0.20 + armSwing && cy < 0.27 + armSwing) {
        if ((cx > 0.18 && cx < 0.21) || (cx > 0.24 && cx < 0.27) ||
            (cx > 0.30 && cx < 0.33)) { *out = 0xE8E8E0; return 1; }
        if ((cx > -0.33 && cx < -0.30) || (cx > -0.27 && cx < -0.24) ||
            (cx > -0.21 && cx < -0.18)) { *out = 0xE8E8E0; return 1; }
    }
    /* legs */
    if (cy > 0.24 && cy < 0.46 && fabs(cx) > 0.05 && fabs(cx) < 0.16) {
        *out = 0x501008; return 1;
    }
    /* hoof claws */
    if (cy > 0.42 && cy < 0.48 && fabs(cx) > 0.04 && fabs(cx) < 0.17) {
        *out = 0x202020; return 1;
    }
    return 0;
}

static void drawEnemy(Enemy *e)
{
    double dx = e->x - g_player.x;
    double dy = e->y - g_player.y;
    double cs = cos(-g_player.angle);
    double sn = sin(-g_player.angle);
    double tx = dx * cs - dy * sn;
    double ty = dx * sn + dy * cs;
    if (tx <= 0.1) return;

    double planeHalf = tan(FOV / 2.0);
    double screenX = (SCREEN_W / 2.0) * (1.0 + ty / (tx * planeHalf));
    int spriteH = (int)((SCREEN_H / tx) * 1.0);
    int spriteW = spriteH;
    int dsx = (int)(screenX - spriteW / 2.0);
    int dsy = -spriteH / 2 + SCREEN_H / 2;
    int sx0 = dsx < 0 ? 0 : dsx;
    int sx1 = (dsx + spriteW) > SCREEN_W ? SCREEN_W : (dsx + spriteW);
    int sy0 = dsy < 0 ? 0 : dsy;
    int sy1 = (dsy + spriteH) > SCREEN_H ? SCREEN_H : (dsy + spriteH);

    double shade = 1.0 - tx / MAX_DEPTH;
    if (shade < 0.25) shade = 0.25;
    int flash = e->hitFlash > 0;

    for (int x = sx0; x < sx1; x++) {
        if (tx >= g_depth[x]) continue;
        double u = (x - dsx) / (double)spriteW;
        for (int y = sy0; y < sy1; y++) {
            double v = (y - dsy) / (double)spriteH;
            uint32_t col = 0;
            int drew = (e->type == EN_IMP) ?
                impPixel(u, v, &col, e->anim) :
                gruntPixel(u, v, &col, e->anim);
            if (!drew) continue;
            uint32_t shaded = flash ? 0xFFF0F0 : shadeColor(col, shade);
            g_pixels[y * SCREEN_W + x] = shaded;
        }
    }
}

static void drawFireball(Fireball *fb)
{
    double dx = fb->x - g_player.x;
    double dy = fb->y - g_player.y;
    double cs = cos(-g_player.angle);
    double sn = sin(-g_player.angle);
    double tx = dx * cs - dy * sn;
    double ty = dx * sn + dy * cs;
    if (tx <= 0.1) return;

    double planeHalf = tan(FOV / 2.0);
    double screenX = (SCREEN_W / 2.0) * (1.0 + ty / (tx * planeHalf));
    int sz = (int)((SCREEN_H / tx) * 0.35);
    if (sz < 2) sz = 2;
    int dsx = (int)(screenX - sz / 2.0);
    int dsy = -sz / 2 + SCREEN_H / 2;
    int sx0 = dsx < 0 ? 0 : dsx;
    int sx1 = (dsx + sz) > SCREEN_W ? SCREEN_W : (dsx + sz);
    int sy0 = dsy < 0 ? 0 : dsy;
    int sy1 = (dsy + sz) > SCREEN_H ? SCREEN_H : (dsy + sz);
    double r2 = (sz * 0.5) * (sz * 0.5);

    for (int x = sx0; x < sx1; x++) {
        if (tx >= g_depth[x]) continue;
        for (int y = sy0; y < sy1; y++) {
            double px = x - (dsx + sz * 0.5);
            double py = y - (dsy + sz * 0.5);
            double d2 = px * px + py * py;
            if (d2 > r2) continue;
            double t = d2 / r2;
            int r = (int)(255 * (1.0 - t * 0.4));
            int g = (int)(180 * (1.0 - t));
            int b = (int)(40 * (1.0 - t));
            g_pixels[y * SCREEN_W + x] = makeColor(r, g, b);
        }
    }
}

static void drawPickup(Pickup *p)
{
    double dx = p->x - g_player.x;
    double dy = p->y - g_player.y;
    double cs = cos(-g_player.angle);
    double sn = sin(-g_player.angle);
    double tx = dx * cs - dy * sn;
    double ty = dx * sn + dy * cs;
    if (tx <= 0.1) return;

    double planeHalf = tan(FOV / 2.0);
    double screenX = (SCREEN_W / 2.0) * (1.0 + ty / (tx * planeHalf));
    int sz = (int)((SCREEN_H / tx) * 0.45);
    if (sz < 4) sz = 4;
    int dsx = (int)(screenX - sz / 2.0);
    double bob = sin(g_globalTime * 3.0 + p->x + p->y) * (sz * 0.08);
    int dsy = (int)(SCREEN_H / 2 + sz * 0.15 + bob);
    int sx0 = dsx < 0 ? 0 : dsx;
    int sx1 = (dsx + sz) > SCREEN_W ? SCREEN_W : (dsx + sz);
    int sy0 = dsy < 0 ? 0 : dsy;
    int sy1 = (dsy + sz) > SCREEN_H ? SCREEN_H : (dsy + sz);

    double shade = 1.0 - tx / MAX_DEPTH;
    if (shade < 0.3) shade = 0.3;

    for (int x = sx0; x < sx1; x++) {
        if (tx >= g_depth[x]) continue;
        double u = (x - dsx) / (double)sz;
        for (int y = sy0; y < sy1; y++) {
            double v = (y - dsy) / (double)sz;
            double cx = u - 0.5, cy = v - 0.5;
            uint32_t col = 0;
            int draw = 0;
            if (p->type == PU_HEALTH) {
                /* white kit with red cross */
                if (fabs(cx) < 0.45 && fabs(cy) < 0.45) {
                    col = 0xE8E8E8; draw = 1;
                    if ((fabs(cx) < 0.10 && fabs(cy) < 0.35) ||
                        (fabs(cy) < 0.10 && fabs(cx) < 0.35))
                        col = 0xD03020;
                    if (fabs(cx) > 0.42 || fabs(cy) > 0.42) col = 0x808080;
                }
            } else {
                /* ammo box: dark green with yellow strap */
                if (fabs(cx) < 0.45 && fabs(cy) < 0.30) {
                    col = 0x305020; draw = 1;
                    if (fabs(cy) < 0.06) col = 0xC0A030;
                    if (fabs(cx) > 0.42 || fabs(cy) > 0.27) col = 0x102008;
                }
            }
            if (draw) g_pixels[y * SCREEN_W + x] = shadeColor(col, shade);
        }
    }
}

static void drawParticles(void)
{
    for (int i = 0; i < MAX_PARTICLES; i++) {
        Particle *p = &g_parts[i];
        if (p->life <= 0) continue;
        double dx = p->x - g_player.x;
        double dy = p->y - g_player.y;
        double cs = cos(-g_player.angle);
        double sn = sin(-g_player.angle);
        double tx = dx * cs - dy * sn;
        double ty = dx * sn + dy * cs;
        if (tx <= 0.1) continue;
        double planeHalf = tan(FOV / 2.0);
        double screenX = (SCREEN_W / 2.0) * (1.0 + ty / (tx * planeHalf));
        int sz = (int)((SCREEN_H / tx) * 0.08);
        if (sz < 1) sz = 1;
        int sx = (int)screenX;
        int sy = SCREEN_H / 2;
        if (sx < 0 || sx >= SCREEN_W) continue;
        if (tx >= g_depth[sx]) continue;
        double fade = p->life;
        if (fade > 1.0) fade = 1.0;
        uint32_t c = shadeColor(p->color, fade);
        for (int yy = -sz; yy <= sz; yy++) {
            for (int xx = -sz; xx <= sz; xx++) {
                if (xx * xx + yy * yy > sz * sz) continue;
                putPixel(sx + xx, sy + yy, c);
            }
        }
    }
}

/* ========================================================================
 * Sprite sort + render
 * ======================================================================== */

static void renderSprites(void)
{
    typedef struct { double d2; int kind; int idx; } Ref;
    Ref refs[MAX_ENEMIES + MAX_PICKUPS + MAX_FIREBALLS];
    int n = 0;
    for (int i = 0; i < MAX_ENEMIES; i++) {
        Enemy *e = &g_enemies[i];
        if (!(e->alive || e->hitFlash > 0)) continue;
        double dx = e->x - g_player.x, dy = e->y - g_player.y;
        refs[n].d2 = dx * dx + dy * dy;
        refs[n].kind = 0; refs[n].idx = i; n++;
    }
    for (int i = 0; i < MAX_PICKUPS; i++) {
        if (!g_pickups[i].alive) continue;
        double dx = g_pickups[i].x - g_player.x;
        double dy = g_pickups[i].y - g_player.y;
        refs[n].d2 = dx * dx + dy * dy;
        refs[n].kind = 1; refs[n].idx = i; n++;
    }
    for (int i = 0; i < MAX_FIREBALLS; i++) {
        if (!g_fireballs[i].alive) continue;
        double dx = g_fireballs[i].x - g_player.x;
        double dy = g_fireballs[i].y - g_player.y;
        refs[n].d2 = dx * dx + dy * dy;
        refs[n].kind = 2; refs[n].idx = i; n++;
    }
    for (int i = 0; i < n; i++) {
        for (int j = i + 1; j < n; j++) {
            if (refs[j].d2 > refs[i].d2) {
                Ref t = refs[i]; refs[i] = refs[j]; refs[j] = t;
            }
        }
    }
    for (int i = 0; i < n; i++) {
        if (refs[i].kind == 0)      drawEnemy(&g_enemies[refs[i].idx]);
        else if (refs[i].kind == 1) drawPickup(&g_pickups[refs[i].idx]);
        else                        drawFireball(&g_fireballs[refs[i].idx]);
    }
    drawParticles();
}

/* ========================================================================
 * HUD
 * ======================================================================== */

static void drawWeapon(void)
{
    int gx = SCREEN_W / 2;
    int gy = SCREEN_H - 40;

    /* stock */
    fillRect(gx - 55, gy - 40, 110, 50, 0x281810);
    fillRect(gx - 55, gy - 40, 110, 4, 0x60381C);
    fillRect(gx - 55, gy - 8, 110, 4, 0x18100A);
    /* receiver */
    fillRect(gx - 45, gy - 60, 90, 25, 0x383838);
    fillRect(gx - 45, gy - 60, 90, 4, 0x585858);
    fillRect(gx - 45, gy - 39, 90, 4, 0x181818);
    /* barrel */
    fillRect(gx - 10, gy - 110, 20, 55, 0x202020);
    fillRect(gx - 10, gy - 110, 4, 55, 0x404040);
    fillRect(gx + 6,  gy - 110, 4, 55, 0x101010);
    /* muzzle */
    fillRect(gx - 12, gy - 114, 24, 6, 0x101010);
    /* pump */
    fillRect(gx - 18, gy - 50, 36, 12, 0x402010);
    fillRect(gx - 18, gy - 50, 36, 3, 0x804030);
    /* sight */
    fillRect(gx - 1, gy - 116, 2, 4, 0xC0C0C0);
    /* trigger guard */
    fillRect(gx - 8, gy - 30, 16, 12, 0x202020);

    if (g_muzzleFlash > 0) {
        int fx = gx, fy = gy - 116;
        for (int y = -25; y < 18; y++) {
            for (int x = -32; x < 32; x++) {
                int d2 = x * x + y * y;
                if (d2 > 700) continue;
                int px = fx + x, py = fy + y;
                if ((unsigned)px >= SCREEN_W || (unsigned)py >= SCREEN_H) continue;
                int d = (int)sqrt((double)d2);
                int v = 255 - d * 9;
                if (v < 0) continue;
                int r = v;
                int gC = (v > 180) ? v : (v * 7 / 10);
                int b = v / 6;
                g_pixels[py * SCREEN_W + px] = makeColor(r, gC, b);
            }
        }
    }
}

static const uint8_t glyph[10][5] = {
    {0x7,0x5,0x5,0x5,0x7}, {0x2,0x6,0x2,0x2,0x7}, {0x7,0x1,0x7,0x4,0x7},
    {0x7,0x1,0x7,0x1,0x7}, {0x5,0x5,0x7,0x1,0x1}, {0x7,0x4,0x7,0x1,0x7},
    {0x7,0x4,0x7,0x5,0x7}, {0x7,0x1,0x1,0x1,0x1}, {0x7,0x5,0x7,0x5,0x7},
    {0x7,0x5,0x7,0x1,0x7},
};

static void drawDigit(int d, int x, int y, uint32_t c)
{
    if (d < 0 || d > 9) return;
    for (int ry = 0; ry < 5; ry++)
        for (int rx = 0; rx < 3; rx++)
            if (glyph[d][ry] & (1 << (2 - rx)))
                fillRect(x + rx * 3, y + ry * 3, 3, 3, c);
}

static void drawNumber(int n, int x, int y, uint32_t c)
{
    if (n < 0) n = 0;
    char buf[16];
    snprintf(buf, sizeof(buf), "%d", n);
    int len = (int)strlen(buf);
    for (int i = 0; i < len; i++)
        drawDigit(buf[i] - '0', x + i * 12, y, c);
}

/* Tiny 3x5 alpha glyphs for HUD labels */
static const uint16_t alphaGlyph[26] = {
    /* A */ 0x57DC, /* B */ 0x7BBC, /* C */ 0x7244, /* D */ 0x7BBC,
    /* E */ 0x7344, /* F */ 0x7340, /* G */ 0x725C, /* H */ 0x5BDC,
    /* I */ 0x7248, /* J */ 0x124C, /* K */ 0x5B5C, /* L */ 0x4924,
    /* M */ 0x5FDC, /* N */ 0x5DDC, /* O */ 0x7BDC, /* P */ 0x7BC8,
    /* Q */ 0x7BDE, /* R */ 0x7B5C, /* S */ 0x711C, /* T */ 0x7248,
    /* U */ 0x5B5C, /* V */ 0x5B54, /* W */ 0x5BFC, /* X */ 0x5B5C,
    /* Y */ 0x5B48, /* Z */ 0x7248,
};

static void drawLetter(char ch, int x, int y, uint32_t c)
{
    if (ch >= 'a' && ch <= 'z') ch -= 32;
    if (ch < 'A' || ch > 'Z') return;
    uint16_t g = alphaGlyph[ch - 'A'];
    for (int ry = 0; ry < 5; ry++) {
        for (int rx = 0; rx < 3; rx++) {
            int bit = 14 - (ry * 3 + rx);
            if (bit >= 0 && (g & (1 << bit)))
                fillRect(x + rx * 2, y + ry * 2, 2, 2, c);
        }
    }
}

static void drawText(const char *s, int x, int y, uint32_t c)
{
    int dx = 0;
    while (*s) {
        if (*s == ' ') { dx += 4; }
        else { drawLetter(*s, x + dx, y, c); dx += 8; }
        s++;
    }
}

static void drawFace(int x, int y, int hp)
{
    /* 28x28 face block */
    uint32_t skin = (hp > 60) ? 0xE0B080 : (hp > 30 ? 0xC09060 : 0x806040);
    uint32_t blood = 0x800000;
    /* head */
    fillRect(x, y, 28, 28, skin);
    fillRect(x, y, 28, 2, 0xA08060);
    fillRect(x, y + 26, 28, 2, 0x604030);
    /* hair */
    fillRect(x + 2, y, 24, 5, 0x402008);
    fillRect(x + 2, y + 4, 4, 2, 0x402008);
    fillRect(x + 22, y + 4, 4, 2, 0x402008);
    /* eyes */
    int eyeY = (hp < 30) ? y + 11 : y + 9;
    fillRect(x + 7, eyeY, 4, 3, 0xFFFFFF);
    fillRect(x + 17, eyeY, 4, 3, 0xFFFFFF);
    int pupilOff = (int)(sin(g_globalTime * 1.7) * 1.0);
    fillRect(x + 8 + pupilOff, eyeY, 2, 3, 0x000000);
    fillRect(x + 18 + pupilOff, eyeY, 2, 3, 0x000000);
    /* nose */
    fillRect(x + 13, y + 13, 2, 4, 0xA07050);
    /* mouth depends on health */
    if (hp > 60) {
        fillRect(x + 9, y + 20, 10, 2, 0x401010);
    } else if (hp > 30) {
        fillRect(x + 10, y + 21, 8, 2, 0x301010);
    } else {
        fillRect(x + 9, y + 22, 10, 2, 0x200808);
        fillRect(x + 9, y + 20, 2, 2, 0x301010);
        fillRect(x + 17, y + 20, 2, 2, 0x301010);
    }
    /* blood for low health */
    if (hp < 50) {
        fillRect(x + 5, y + 6, 2, 4, blood);
        fillRect(x + 21, y + 8, 2, 6, blood);
    }
    if (hp < 25) {
        fillRect(x + 12, y + 5, 4, 3, blood);
        fillRect(x + 10, y + 8, 2, 3, blood);
    }
    if (hp <= 0) {
        /* X over eyes */
        for (int i = 0; i < 4; i++) {
            putPixel(x + 7 + i, eyeY + i, 0x000000);
            putPixel(x + 10 - i, eyeY + i, 0x000000);
            putPixel(x + 17 + i, eyeY + i, 0x000000);
            putPixel(x + 20 - i, eyeY + i, 0x000000);
        }
    }
}

static void drawHUD(void)
{
    int barY = SCREEN_H - 56;
    /* gradient background bar */
    for (int y = barY; y < SCREEN_H; y++) {
        int t = (y - barY) * 255 / 56;
        uint32_t c = makeColor(32 + t / 8, 28 + t / 8, 24 + t / 10);
        fillRect(0, y, SCREEN_W, 1, c);
    }
    fillRect(0, barY, SCREEN_W, 2, 0xA08060);
    fillRect(0, barY + 2, SCREEN_W, 1, 0x402010);

    /* dividers */
    for (int dx = 110; dx <= SCREEN_W - 110; dx += SCREEN_W - 220) {
        fillRect(dx, barY + 6, 2, 44, 0x281810);
        fillRect(dx + 2, barY + 6, 1, 44, 0x60381C);
    }

    /* Face panel center */
    int fx = SCREEN_W / 2 - 14;
    int fy = barY + 14;
    fillRect(fx - 4, fy - 4, 36, 36, 0x100804);
    drawFace(fx, fy, g_player.health);

    /* Health */
    uint32_t hc = g_player.health > 50 ? 0x40E040
                : g_player.health > 20 ? 0xE0E040 : 0xE04040;
    drawText("HEALTH", 20, barY + 6, 0xC0A080);
    drawNumber(g_player.health, 20, barY + 18, hc);

    /* Ammo */
    drawText("AMMO", SCREEN_W - 80, barY + 6, 0xC0A080);
    drawNumber(g_player.ammo, SCREEN_W - 80, barY + 18, 0xE0E060);

    /* Level + kills */
    int kills = 0, total = 0;
    for (int i = 0; i < MAX_ENEMIES; i++) {
        Enemy *e = &g_enemies[i];
        if (e->alive || e->hitFlash > 0 || e->hp <= 0) {
            total++;
            if (!e->alive) kills++;
        }
    }
    char buf[32];
    snprintf(buf, sizeof(buf), "LEVEL %d", g_level + 1);
    drawText(buf, 20, SCREEN_H - 14, 0xE0C080);

    char buf2[32];
    snprintf(buf2, sizeof(buf2), "KILLS %d %d", kills, total);
    drawText(buf2, SCREEN_W - 110, SCREEN_H - 14, 0xC0A080);
}

static void drawCrosshair(void)
{
    int cx = SCREEN_W / 2, cy = SCREEN_H / 2 - 30;
    for (int i = -5; i <= 5; i++) {
        if (i >= -1 && i <= 1) continue;
        putPixel(cx + i, cy, 0xE0E0E0);
        putPixel(cx, cy + i, 0xE0E0E0);
    }
    putPixel(cx, cy, 0xFF4040);
}

static void drawBanner(const char *text, int y, uint32_t c)
{
    fillRect(0, y - 10, SCREEN_W, 3, c);
    fillRect(0, y + 9, SCREEN_W, 3, c);
    int tw = (int)strlen(text) * 8;
    int tx = (SCREEN_W - tw) / 2;
    drawText(text, tx, y - 5, c);
}

static void drawIntro(void)
{
    int x0 = 70, y0 = 50, w = SCREEN_W - 140, h = 280;

    /* dim and tint background panel */
    for (int y = y0; y < y0 + h; y++) {
        for (int x = x0; x < x0 + w; x++) {
            uint32_t c = g_pixels[y * SCREEN_W + x];
            int r = (c >> 16) & 0xFF;
            int g = (c >> 8) & 0xFF;
            int b = c & 0xFF;
            g_pixels[y * SCREEN_W + x] = makeColor(r / 4 + 10, g / 5, b / 5);
        }
    }

    /* double-line border */
    fillRect(x0, y0, w, 3, 0xC0A040);
    fillRect(x0, y0 + h - 3, w, 3, 0xC0A040);
    fillRect(x0, y0, 3, h, 0xC0A040);
    fillRect(x0 + w - 3, y0, 3, h, 0xC0A040);
    fillRect(x0 + 6, y0 + 6, w - 12, 1, 0x603018);
    fillRect(x0 + 6, y0 + h - 7, w - 12, 1, 0x603018);
    fillRect(x0 + 6, y0 + 6, 1, h - 12, 0x603018);
    fillRect(x0 + w - 7, y0 + 6, 1, h - 12, 0x603018);

    /* title */
    drawText("DOOM CLONE", x0 + (w - 10 * 8) / 2, y0 + 20, 0xFFC040);
    drawText("CONTROLS",   x0 + (w - 8 * 8) / 2,  y0 + 56, 0xE0E0E0);

    int lx = x0 + 60;
    int ty = y0 + 90;
    drawText("W S",       lx,        ty, 0x60C0FF);
    drawText("MOVE",      lx + 160,  ty, 0xC0C0C0); ty += 22;
    drawText("A D",       lx,        ty, 0x60C0FF);
    drawText("STRAFE",    lx + 160,  ty, 0xC0C0C0); ty += 22;
    drawText("ARROWS",    lx,        ty, 0x60C0FF);
    drawText("TURN",      lx + 160,  ty, 0xC0C0C0); ty += 22;
    drawText("SPACE",     lx,        ty, 0x60C0FF);
    drawText("SHOOT",     lx + 160,  ty, 0xC0C0C0); ty += 22;
    drawText("R",         lx,        ty, 0x60C0FF);
    drawText("RESTART",   lx + 160,  ty, 0xC0C0C0); ty += 22;
    drawText("ESC",       lx,        ty, 0x60C0FF);
    drawText("QUIT",      lx + 160,  ty, 0xC0C0C0); ty += 28;

    /* blinking prompt */
    if (((int)(g_globalTime * 2.0)) & 1) {
        drawText("PRESS ANY KEY", x0 + (w - 13 * 8) / 2, ty, 0x40E040);
    }
}

static void drawMinimap(void)
{
    int mx0 = SCREEN_W - 100, my0 = 8;
    int cell = 5;
    fillRect(mx0 - 2, my0 - 2, MAP_W * cell + 4, MAP_H * cell + 4, 0x101010);
    for (int y = 0; y < MAP_H; y++) {
        for (int x = 0; x < MAP_W; x++) {
            uint32_t c;
            char ch = g_curMap[y][x];
            if (ch == '.') c = 0x303030;
            else if (ch == '#') c = 0x808080;
            else if (ch == '=') c = 0xA04030;
            else if (ch == 'B') c = 0x4060A0;
            else if (ch == 'D') c = 0x805020;
            else c = 0x404040;
            fillRect(mx0 + x * cell, my0 + y * cell, cell - 1, cell - 1, c);
        }
    }
    /* enemies */
    for (int i = 0; i < MAX_ENEMIES; i++) {
        if (!g_enemies[i].alive) continue;
        uint32_t c = (g_enemies[i].type == EN_IMP) ? 0xE04020 : 0xC0C040;
        int px = mx0 + (int)(g_enemies[i].x * cell);
        int py = my0 + (int)(g_enemies[i].y * cell);
        fillRect(px - 1, py - 1, 3, 3, c);
    }
    /* pickups */
    for (int i = 0; i < MAX_PICKUPS; i++) {
        if (!g_pickups[i].alive) continue;
        uint32_t c = (g_pickups[i].type == PU_HEALTH) ? 0xE04040 : 0xE0C040;
        int px = mx0 + (int)(g_pickups[i].x * cell);
        int py = my0 + (int)(g_pickups[i].y * cell);
        putPixel(px, py, c);
        putPixel(px + 1, py, c);
        putPixel(px, py + 1, c);
    }
    /* player */
    int ppx = mx0 + (int)(g_player.x * cell);
    int ppy = my0 + (int)(g_player.y * cell);
    fillRect(ppx - 1, ppy - 1, 3, 3, 0x40E040);
    int dx = (int)(cos(g_player.angle) * 4);
    int dy = (int)(sin(g_player.angle) * 4);
    for (int s = 0; s < 4; s++) {
        putPixel(ppx + dx * s / 4, ppy + dy * s / 4, 0x80FF80);
    }
}

/* ========================================================================
 * Game logic
 * ======================================================================== */

static void tryMove(double nx, double ny)
{
    double pad = 0.18;
    if (!mapBlocked((int)(nx + pad), (int)g_player.y) &&
        !mapBlocked((int)(nx - pad), (int)g_player.y))
        g_player.x = nx;
    if (!mapBlocked((int)g_player.x, (int)(ny + pad)) &&
        !mapBlocked((int)g_player.x, (int)(ny - pad)))
        g_player.y = ny;
}

static void spawnParticle(double x, double y, double vx, double vy,
                          double life, uint32_t color)
{
    for (int i = 0; i < MAX_PARTICLES; i++) {
        if (g_parts[i].life <= 0) {
            g_parts[i].x = x; g_parts[i].y = y;
            g_parts[i].vx = vx; g_parts[i].vy = vy;
            g_parts[i].life = life;
            g_parts[i].color = color;
            return;
        }
    }
}

static void spawnBlood(double x, double y, int count)
{
    for (int i = 0; i < count; i++) {
        double a = (rand() / (double)RAND_MAX) * 2 * M_PI;
        double s = 0.3 + (rand() / (double)RAND_MAX) * 0.7;
        spawnParticle(x, y, cos(a) * s, sin(a) * s,
                      0.6 + (rand() / (double)RAND_MAX) * 0.4,
                      0xC02020);
    }
}

static void spawnSparks(double x, double y)
{
    for (int i = 0; i < 6; i++) {
        double a = (rand() / (double)RAND_MAX) * 2 * M_PI;
        spawnParticle(x, y, cos(a) * 0.4, sin(a) * 0.4,
                      0.35, 0xFFE060);
    }
}

static void updateParticles(double dt)
{
    for (int i = 0; i < MAX_PARTICLES; i++) {
        if (g_parts[i].life <= 0) continue;
        g_parts[i].life -= dt;
        g_parts[i].x += g_parts[i].vx * dt;
        g_parts[i].y += g_parts[i].vy * dt;
        g_parts[i].vx *= 0.92;
        g_parts[i].vy *= 0.92;
    }
}

static void spawnFireball(double x, double y, double tx, double ty)
{
    for (int i = 0; i < MAX_FIREBALLS; i++) {
        if (g_fireballs[i].alive) continue;
        double dx = tx - x, dy = ty - y;
        double d = sqrt(dx * dx + dy * dy);
        if (d < 0.0001) return;
        g_fireballs[i].x = x;
        g_fireballs[i].y = y;
        g_fireballs[i].vx = dx / d * 3.0;
        g_fireballs[i].vy = dy / d * 3.0;
        g_fireballs[i].alive = 1;
        g_fireballs[i].life = 3.0;
        return;
    }
}

static void updateFireballs(double dt)
{
    for (int i = 0; i < MAX_FIREBALLS; i++) {
        Fireball *fb = &g_fireballs[i];
        if (!fb->alive) continue;
        fb->life -= dt;
        if (fb->life <= 0) { fb->alive = 0; continue; }
        double nx = fb->x + fb->vx * dt;
        double ny = fb->y + fb->vy * dt;
        if (mapBlocked((int)nx, (int)ny)) {
            spawnSparks(fb->x, fb->y);
            fb->alive = 0;
            continue;
        }
        fb->x = nx; fb->y = ny;
        /* hit player */
        double dx = g_player.x - fb->x;
        double dy = g_player.y - fb->y;
        if (dx * dx + dy * dy < 0.18) {
            g_player.health -= 12;
            if (g_player.health < 0) g_player.health = 0;
            g_painFlash = 0.35;
            spawnBlood(fb->x, fb->y, 6);
            fb->alive = 0;
        }
        /* spawn flame trail */
        if (((int)(g_globalTime * 30)) % 2 == 0) {
            spawnParticle(fb->x, fb->y, 0, 0, 0.25, 0xFFA040);
        }
    }
}

static void updateEnemies(double dt)
{
    for (int i = 0; i < MAX_ENEMIES; i++) {
        Enemy *e = &g_enemies[i];
        if (e->hitFlash > 0) e->hitFlash -= dt;
        if (!e->alive) continue;
        e->anim += dt * 4.0;
        if (e->atkCool > 0) e->atkCool -= dt;

        double dx = g_player.x - e->x;
        double dy = g_player.y - e->y;
        double dist = sqrt(dx * dx + dy * dy);
        if (dist < 0.001) continue;
        double nx = dx / dist;
        double ny = dy / dist;

        if (e->type == EN_GRUNT) {
            double speed = 1.1 * dt;
            if (dist > 0.7) {
                double mx = e->x + nx * speed;
                double my = e->y + ny * speed;
                if (!mapBlocked((int)mx, (int)e->y)) e->x = mx;
                if (!mapBlocked((int)e->x, (int)my)) e->y = my;
            } else if (e->atkCool <= 0 && g_player.health > 0) {
                g_player.health -= 7;
                if (g_player.health < 0) g_player.health = 0;
                e->atkCool = 1.0;
                g_painFlash = 0.3;
            }
        } else {
            /* IMP: keep medium distance, throw fireballs */
            double speed = 0.9 * dt;
            if (dist > 4.5) {
                double mx = e->x + nx * speed;
                double my = e->y + ny * speed;
                if (!mapBlocked((int)mx, (int)e->y)) e->x = mx;
                if (!mapBlocked((int)e->x, (int)my)) e->y = my;
            } else if (dist < 2.5) {
                double mx = e->x - nx * speed * 0.5;
                double my = e->y - ny * speed * 0.5;
                if (!mapBlocked((int)mx, (int)e->y)) e->x = mx;
                if (!mapBlocked((int)e->x, (int)my)) e->y = my;
            }
            if (e->atkCool <= 0 && dist < 8.0 && g_player.health > 0) {
                spawnFireball(e->x, e->y, g_player.x, g_player.y);
                e->atkCool = 2.0 + (rand() / (double)RAND_MAX);
            }
        }
    }
}

static void updatePickups(void)
{
    for (int i = 0; i < MAX_PICKUPS; i++) {
        if (!g_pickups[i].alive) continue;
        double dx = g_pickups[i].x - g_player.x;
        double dy = g_pickups[i].y - g_player.y;
        if (dx * dx + dy * dy < 0.25) {
            if (g_pickups[i].type == PU_HEALTH) {
                g_player.health += 25;
                if (g_player.health > 100) g_player.health = 100;
            } else {
                g_player.ammo += 12;
                if (g_player.ammo > 99) g_player.ammo = 99;
            }
            g_pickups[i].alive = 0;
            for (int k = 0; k < 8; k++) {
                double a = (rand() / (double)RAND_MAX) * 2 * M_PI;
                spawnParticle(g_pickups[i].x, g_pickups[i].y,
                              cos(a) * 0.3, sin(a) * 0.3, 0.4,
                              g_pickups[i].type == PU_HEALTH ? 0xFFC0C0 : 0xFFE060);
            }
        }
    }
}

static int allEnemiesDead(void)
{
    for (int i = 0; i < MAX_ENEMIES; i++)
        if (g_enemies[i].alive) return 0;
    return 1;
}

static void shoot(void)
{
    if (g_player.ammo <= 0) return;
    g_player.ammo--;
    g_muzzleFlash = 5;

    /* Find nearest enemy along the aim ray within angular tolerance */
    double rx = cos(g_player.angle);
    double ry = sin(g_player.angle);

    /* Wall stop distance */
    double wallT = 0;
    while (wallT < MAX_DEPTH) {
        wallT += 0.05;
        if (mapBlocked((int)(g_player.x + rx * wallT),
                       (int)(g_player.y + ry * wallT))) {
            spawnSparks(g_player.x + rx * wallT, g_player.y + ry * wallT);
            break;
        }
    }

    int bestIdx = -1;
    double bestDist = wallT;
    for (int i = 0; i < MAX_ENEMIES; i++) {
        Enemy *e = &g_enemies[i];
        if (!e->alive) continue;
        double dx = e->x - g_player.x;
        double dy = e->y - g_player.y;
        double d = sqrt(dx * dx + dy * dy);
        double ang = atan2(dy, dx) - g_player.angle;
        while (ang > M_PI)  ang -= 2 * M_PI;
        while (ang < -M_PI) ang += 2 * M_PI;
        /* angular tolerance shrinks with distance */
        double tol = 0.22 / (d < 1 ? 1 : d);
        if (tol < 0.04) tol = 0.04;
        if (fabs(ang) > tol) continue;
        if (d < bestDist) { bestDist = d; bestIdx = i; }
    }
    if (bestIdx >= 0) {
        Enemy *e = &g_enemies[bestIdx];
        e->hp--;
        e->hitFlash = 0.15;
        spawnBlood(e->x, e->y, 8);
        if (e->hp <= 0) {
            e->alive = 0;
            spawnBlood(e->x, e->y, 14);
        }
    }
}

static void resetGame(void)
{
    g_player.health = 100;
    g_player.armor  = 0;
    g_player.ammo   = 50;
    loadLevel(0);
}

static void updateGame(double dt)
{
    g_globalTime += dt;
    if (g_painFlash > 0) g_painFlash -= dt;

    if (g_showIntro) {
        if (g_keyEdge[K_QUIT]) { g_running = 0; g_keyEdge[K_QUIT] = 0; return; }
        for (int i = 0; i < K_COUNT; i++) {
            if (i == K_QUIT) continue;
            if (g_keyEdge[i]) { g_showIntro = 0; g_keyEdge[i] = 0; break; }
        }
        return;
    }

    double moveSpeed = 2.6 * dt;
    double turnSpeed = 2.2 * dt;
    double fx = cos(g_player.angle), fy = sin(g_player.angle);
    double sxv = -sin(g_player.angle), syv = cos(g_player.angle);

    if (g_player.health > 0) {
        if (g_keys[K_FWD])     tryMove(g_player.x + fx  * moveSpeed,
                                       g_player.y + fy  * moveSpeed);
        if (g_keys[K_BACK])    tryMove(g_player.x - fx  * moveSpeed,
                                       g_player.y - fy  * moveSpeed);
        if (g_keys[K_STRAFEL]) tryMove(g_player.x + sxv * moveSpeed,
                                       g_player.y + syv * moveSpeed);
        if (g_keys[K_STRAFER]) tryMove(g_player.x - sxv * moveSpeed,
                                       g_player.y - syv * moveSpeed);
        if (g_keys[K_TURNL])   g_player.angle -= turnSpeed;
        if (g_keys[K_TURNR])   g_player.angle += turnSpeed;

        if (g_keyEdge[K_SHOOT]) { shoot(); g_keyEdge[K_SHOOT] = 0; }
    }

    if (g_keyEdge[K_RESTART] && g_player.health <= 0) {
        resetGame();
        g_keyEdge[K_RESTART] = 0;
    }
    if (g_keyEdge[K_QUIT]) g_running = 0;

    if (g_muzzleFlash > 0) g_muzzleFlash--;

    if (g_player.health > 0) {
        updateEnemies(dt);
        updateFireballs(dt);
        updatePickups();
    }
    updateParticles(dt);

    if (g_player.health > 0 && allEnemiesDead()) {
        g_levelClearTimer += dt;
        if (g_levelClearTimer > 2.5) {
            if (g_level + 1 < LEVEL_COUNT) {
                loadLevel(g_level + 1);
            } else {
                /* victory: stay on last level cleared */
                g_levelClearTimer = 0;
            }
        }
    }
}

static void postProcess(void)
{
    if (g_painFlash > 0) {
        double a = g_painFlash;
        if (a > 0.4) a = 0.4;
        for (int i = 0; i < SCREEN_W * SCREEN_H; i++) {
            uint32_t c = g_pixels[i];
            int r = (c >> 16) & 0xFF;
            int g = (c >> 8) & 0xFF;
            int b = c & 0xFF;
            r = (int)(r + (255 - r) * a);
            g = (int)(g * (1 - a * 0.4));
            b = (int)(b * (1 - a * 0.4));
            g_pixels[i] = makeColor(r, g, b);
        }
    }
}

static void renderFrame(void)
{
    for (int x = 0; x < SCREEN_W; x++) castColumn(x);
    renderSprites();
    drawCrosshair();
    drawWeapon();
    drawHUD();
    drawMinimap();
    postProcess();

    if (g_player.health <= 0) {
        for (int i = 0; i < SCREEN_W * (SCREEN_H - 56); i++) {
            uint32_t c = g_pixels[i];
            int r = (c >> 16) & 0xFF;
            int g = (c >> 8) & 0xFF;
            int b = c & 0xFF;
            r = (r + 220) / 2; g /= 3; b /= 3;
            g_pixels[i] = makeColor(r, g, b);
        }
        drawBanner("YOU DIED  PRESS R", SCREEN_H / 2 - 30, 0xFF4040);
    } else if (allEnemiesDead()) {
        if (g_level + 1 < LEVEL_COUNT) {
            drawBanner("LEVEL CLEAR", 60, 0x40FF40);
        } else {
            drawBanner("VICTORY", 60, 0x40E0FF);
        }
    }

    if (g_showIntro) drawIntro();
}

/* ========================================================================
 * Platform layer
 * ======================================================================== */

#ifdef _WIN32

static BITMAPINFO g_bmi;
static int g_rawKeyDown[256];

static int rawToGameKey(unsigned vk, int *out)
{
    switch (vk) {
    case 'W': case VK_UP:    *out = K_FWD;     return 1;
    case 'S': case VK_DOWN:  *out = K_BACK;    return 1;
    case 'A':                *out = K_STRAFEL; return 1;
    case 'D':                *out = K_STRAFER; return 1;
    case VK_LEFT:            *out = K_TURNL;   return 1;
    case VK_RIGHT:           *out = K_TURNR;   return 1;
    case VK_SPACE:           *out = K_SHOOT;   return 1;
    case 'R':                *out = K_RESTART; return 1;
    case VK_ESCAPE:          *out = K_QUIT;    return 1;
    }
    return 0;
}

static LRESULT CALLBACK WndProc(HWND hwnd, UINT msg, WPARAM wp, LPARAM lp)
{
    switch (msg) {
    case WM_CLOSE:
    case WM_DESTROY:
        g_running = 0;
        PostQuitMessage(0);
        return 0;
    case WM_KEYDOWN: {
        if (wp < 256) {
            int k;
            if (rawToGameKey((unsigned)wp, &k)) {
                if (!g_rawKeyDown[wp]) g_keyEdge[k] = 1;
                g_keys[k] = 1;
            }
            g_rawKeyDown[wp] = 1;
        }
        return 0;
    }
    case WM_KEYUP:
        if (wp < 256) {
            int k;
            if (rawToGameKey((unsigned)wp, &k)) g_keys[k] = 0;
            g_rawKeyDown[wp] = 0;
        }
        return 0;
    case WM_ERASEBKGND:
        return 1;
    case WM_PAINT: {
        PAINTSTRUCT ps;
        HDC dc = BeginPaint(hwnd, &ps);
        RECT rc; GetClientRect(hwnd, &rc);
        StretchDIBits(dc,
            0, 0, rc.right, rc.bottom,
            0, 0, SCREEN_W, SCREEN_H,
            g_pixels, &g_bmi, DIB_RGB_COLORS, SRCCOPY);
        EndPaint(hwnd, &ps);
        return 0;
    }
    }
    return DefWindowProcA(hwnd, msg, wp, lp);
}

int WINAPI WinMain(HINSTANCE hInst, HINSTANCE hPrev, LPSTR cmd, int show)
{
    (void)hPrev; (void)cmd;

    g_pixels = (uint32_t *)VirtualAlloc(NULL, SCREEN_W * SCREEN_H * 4,
                                        MEM_COMMIT | MEM_RESERVE, PAGE_READWRITE);
    if (!g_pixels) return 1;

    ZeroMemory(&g_bmi, sizeof(g_bmi));
    g_bmi.bmiHeader.biSize = sizeof(BITMAPINFOHEADER);
    g_bmi.bmiHeader.biWidth = SCREEN_W;
    g_bmi.bmiHeader.biHeight = -SCREEN_H;
    g_bmi.bmiHeader.biPlanes = 1;
    g_bmi.bmiHeader.biBitCount = 32;
    g_bmi.bmiHeader.biCompression = BI_RGB;

    WNDCLASSA wc = {0};
    wc.lpfnWndProc = WndProc;
    wc.hInstance = hInst;
    wc.hCursor = LoadCursor(NULL, IDC_ARROW);
    wc.lpszClassName = "DoomCloneWnd";
    wc.style = CS_HREDRAW | CS_VREDRAW | CS_OWNDC;
    RegisterClassA(&wc);

    RECT r = {0, 0, SCREEN_W * 2, SCREEN_H * 2};
    DWORD style = WS_OVERLAPPEDWINDOW & ~(WS_THICKFRAME | WS_MAXIMIZEBOX);
    AdjustWindowRect(&r, style, FALSE);

    HWND hwnd = CreateWindowA("DoomCloneWnd", "Doom Clone",
        style | WS_VISIBLE,
        CW_USEDEFAULT, CW_USEDEFAULT,
        r.right - r.left, r.bottom - r.top,
        NULL, NULL, hInst, NULL);
    if (!hwnd) return 1;
    ShowWindow(hwnd, show);

    LARGE_INTEGER freq, prev;
    QueryPerformanceFrequency(&freq);
    QueryPerformanceCounter(&prev);

    buildTextures();
    resetGame();

    MSG msg;
    while (g_running) {
        while (PeekMessageA(&msg, NULL, 0, 0, PM_REMOVE)) {
            if (msg.message == WM_QUIT) { g_running = 0; break; }
            TranslateMessage(&msg);
            DispatchMessageA(&msg);
        }
        if (!g_running) break;

        LARGE_INTEGER now; QueryPerformanceCounter(&now);
        double dt = (now.QuadPart - prev.QuadPart) / (double)freq.QuadPart;
        if (dt > 0.05) dt = 0.05;
        prev = now;

        updateGame(dt);
        renderFrame();
        InvalidateRect(hwnd, NULL, FALSE);
        UpdateWindow(hwnd);

        Sleep(1);
    }
    return 0;
}

#else  /* POSIX / X11 */

static int xkeyToGameKey(KeySym ks, int *out)
{
    switch (ks) {
    case XK_w: case XK_W: case XK_Up:    *out = K_FWD;     return 1;
    case XK_s: case XK_S: case XK_Down:  *out = K_BACK;    return 1;
    case XK_a: case XK_A:                *out = K_STRAFEL; return 1;
    case XK_d: case XK_D:                *out = K_STRAFER; return 1;
    case XK_Left:                        *out = K_TURNL;   return 1;
    case XK_Right:                       *out = K_TURNR;   return 1;
    case XK_space:                       *out = K_SHOOT;   return 1;
    case XK_r: case XK_R:                *out = K_RESTART; return 1;
    case XK_Escape:                      *out = K_QUIT;    return 1;
    }
    return 0;
}

static double nowSec(void)
{
    struct timespec ts;
    clock_gettime(CLOCK_MONOTONIC, &ts);
    return ts.tv_sec + ts.tv_nsec / 1e9;
}

int main(int argc, char **argv)
{
    int headless = 0;
    int maxFrames = -1;
    for (int i = 1; i < argc; i++) {
        if (strcmp(argv[i], "--headless") == 0) headless = 1;
        else if (strcmp(argv[i], "--frames") == 0 && i + 1 < argc) {
            maxFrames = atoi(argv[++i]);
        }
    }

    g_pixels = (uint32_t *)calloc((size_t)SCREEN_W * SCREEN_H, 4);
    if (!g_pixels) return 1;

    Display *dpy = NULL;
    Window win = 0;
    GC gc = 0;
    XImage *img = NULL;
    Atom wmDelete = 0;
    int screen = 0;

    if (!headless) {
        dpy = XOpenDisplay(NULL);
        if (!dpy) {
            fprintf(stderr, "XOpenDisplay failed (no DISPLAY?). Try --headless.\n");
            return 1;
        }
        screen = DefaultScreen(dpy);
        Window root = RootWindow(dpy, screen);
        int winW = SCREEN_W * 2, winH = SCREEN_H * 2;
        win = XCreateSimpleWindow(dpy, root, 0, 0, winW, winH, 0,
                                  BlackPixel(dpy, screen), BlackPixel(dpy, screen));
        XStoreName(dpy, win, "Doom Clone");
        XSelectInput(dpy, win, ExposureMask | KeyPressMask | KeyReleaseMask | StructureNotifyMask);

        wmDelete = XInternAtom(dpy, "WM_DELETE_WINDOW", False);
        XSetWMProtocols(dpy, win, &wmDelete, 1);

        XMapWindow(dpy, win);
        gc = XCreateGC(dpy, win, 0, NULL);

        Visual *vis = DefaultVisual(dpy, screen);
        int depth = DefaultDepth(dpy, screen);
        img = XCreateImage(dpy, vis, depth, ZPixmap, 0,
                           (char *)g_pixels, SCREEN_W, SCREEN_H, 32, 0);
        if (!img) {
            fprintf(stderr, "XCreateImage failed\n");
            return 1;
        }
    }

    buildTextures();
    resetGame();

    double prev = nowSec();
    int frames = 0;

    while (g_running) {
        if (!headless) {
            while (XPending(dpy)) {
                XEvent ev;
                XNextEvent(dpy, &ev);
                if (ev.type == KeyPress || ev.type == KeyRelease) {
                    KeySym ks = XLookupKeysym(&ev.xkey, 0);
                    int k;
                    if (xkeyToGameKey(ks, &k)) {
                        if (ev.type == KeyPress) {
                            if (!g_keys[k]) g_keyEdge[k] = 1;
                            g_keys[k] = 1;
                        } else {
                            if (XEventsQueued(dpy, QueuedAfterReading)) {
                                XEvent nxt;
                                XPeekEvent(dpy, &nxt);
                                if (nxt.type == KeyPress &&
                                    nxt.xkey.time == ev.xkey.time &&
                                    nxt.xkey.keycode == ev.xkey.keycode) {
                                    XNextEvent(dpy, &nxt);
                                    continue;
                                }
                            }
                            g_keys[k] = 0;
                        }
                    }
                } else if (ev.type == ClientMessage) {
                    if ((Atom)ev.xclient.data.l[0] == wmDelete) g_running = 0;
                }
            }
        }

        double n = nowSec();
        double dt = n - prev;
        if (dt > 0.05) dt = 0.05;
        prev = n;

        updateGame(dt);
        renderFrame();

        if (!headless) {
            XPutImage(dpy, win, gc, img, 0, 0, 0, 0, SCREEN_W, SCREEN_H);
            XFlush(dpy);
        }

        frames++;
        if (maxFrames > 0 && frames >= maxFrames) g_running = 0;

        struct timespec ts = {0, 16 * 1000 * 1000};
        nanosleep(&ts, NULL);
    }

    if (!headless) {
        img->data = NULL;
        XDestroyImage(img);
        XFreeGC(dpy, gc);
        XDestroyWindow(dpy, win);
        XCloseDisplay(dpy);
    }
    free(g_pixels);
    return 0;
}

#endif
