/*
 * doom.c - Minimal DOOM-style raycasting FPS for Win32 (x64 / ARM64).
 * Single file, no external dependencies beyond the Windows SDK.
 *
 * Controls:
 *   W / Up      - move forward
 *   S / Down    - move backward
 *   A           - strafe left
 *   D           - strafe right
 *   Left/Right  - turn
 *   Space       - shoot
 *   Esc         - quit
 */

#define WIN32_LEAN_AND_MEAN
#include <windows.h>
#include <math.h>
#include <stdint.h>
#include <stdio.h>
#include <string.h>

#ifndef M_PI
#define M_PI 3.14159265358979323846
#endif

#define SCREEN_W 640
#define SCREEN_H 400
#define MAP_W    16
#define MAP_H    16
#define FOV      (M_PI / 3.0)
#define MAX_DEPTH 24.0

static const char *g_map[MAP_H] = {
    "################",
    "#..............#",
    "#..##....####..#",
    "#..#.......#...#",
    "#..#.......#...#",
    "#..####....#...#",
    "#..............#",
    "#......#####...#",
    "#......#.......#",
    "#......#...##..#",
    "#......#.......#",
    "#......#########",
    "#..............#",
    "#..###.........#",
    "#..............#",
    "################",
};

typedef struct {
    double x, y;
    double angle;
    int health;
    int ammo;
} Player;

typedef struct {
    double x, y;
    int alive;
    double hitFlash;
} Enemy;

static Player g_player;
static Enemy  g_enemy;

static uint32_t *g_pixels;
static BITMAPINFO g_bmi;
static double g_depth[SCREEN_W];

static int g_keys[256];
static int g_keyEdge[256];
static int g_running = 1;
static int g_muzzleFlash = 0;

static LARGE_INTEGER g_freq;

static int mapCell(int mx, int my)
{
    if (mx < 0 || mx >= MAP_W || my < 0 || my >= MAP_H) return 1;
    return g_map[my][mx] == '#';
}

static uint32_t shadeWall(int side, double dist)
{
    double t = dist / MAX_DEPTH;
    if (t > 1.0) t = 1.0;
    double bright = 1.0 - t;
    if (bright < 0.15) bright = 0.15;
    int base = side ? 140 : 200;
    int r = (int)(base * bright);
    int g = (int)((base * 0.35) * bright);
    int b = (int)((base * 0.20) * bright);
    return (r << 16) | (g << 8) | b;
}

static uint32_t shadeFloor(double dist)
{
    double t = dist / MAX_DEPTH;
    if (t > 1.0) t = 1.0;
    double b = 1.0 - t;
    if (b < 0.05) b = 0.05;
    int v = (int)(80 * b);
    return (v << 16) | (v << 8) | (v / 2);
}

static uint32_t shadeCeil(double dist)
{
    double t = dist / MAX_DEPTH;
    if (t > 1.0) t = 1.0;
    double b = 1.0 - t;
    if (b < 0.05) b = 0.05;
    int v = (int)(60 * b);
    return (v << 16) | (v << 8) | v;
}

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

    int hit = 0, side = 0, iter = 0;
    while (!hit && iter++ < 128) {
        if (sideX < sideY) {
            sideX += deltaX; mapX += stepX; side = 0;
        } else {
            sideY += deltaY; mapY += stepY; side = 1;
        }
        if (mapCell(mapX, mapY)) hit = 1;
    }

    double perpDist;
    if (side == 0) perpDist = (sideX - deltaX);
    else           perpDist = (sideY - deltaY);
    if (perpDist < 0.0001) perpDist = 0.0001;

    g_depth[col] = perpDist;

    int lineH = (int)(SCREEN_H / perpDist);
    int drawStart = -lineH / 2 + SCREEN_H / 2;
    int drawEnd   =  lineH / 2 + SCREEN_H / 2;
    if (drawStart < 0) drawStart = 0;
    if (drawEnd >= SCREEN_H) drawEnd = SCREEN_H - 1;

    uint32_t wallColor = shadeWall(side, perpDist);

    /* Vertical "brick" stripe pattern */
    double wallHitX;
    if (side == 0) wallHitX = g_player.y + perpDist * rayDirY;
    else           wallHitX = g_player.x + perpDist * rayDirX;
    wallHitX -= floor(wallHitX);
    int stripe = (int)(wallHitX * 8.0);
    if (stripe == 0 || stripe == 4) {
        int r = (wallColor >> 16) & 0xFF;
        int g = (wallColor >> 8)  & 0xFF;
        int b =  wallColor        & 0xFF;
        r = r * 3 / 4; g = g * 3 / 4; b = b * 3 / 4;
        wallColor = (r << 16) | (g << 8) | b;
    }

    /* Ceiling */
    for (int y = 0; y < drawStart; y++) {
        double rowDist = SCREEN_H / (double)(SCREEN_H - 2 * y);
        if (rowDist < 0) rowDist = MAX_DEPTH;
        g_pixels[y * SCREEN_W + col] = shadeCeil(rowDist);
    }
    /* Wall */
    for (int y = drawStart; y <= drawEnd; y++) {
        g_pixels[y * SCREEN_W + col] = wallColor;
    }
    /* Floor */
    for (int y = drawEnd + 1; y < SCREEN_H; y++) {
        double rowDist = SCREEN_H / (double)(2 * y - SCREEN_H);
        if (rowDist < 0) rowDist = MAX_DEPTH;
        g_pixels[y * SCREEN_W + col] = shadeFloor(rowDist);
    }
}

static void drawSprite(double sx, double sy, double scale, uint32_t tint)
{
    /* Transform sprite position to camera space */
    double dx = sx - g_player.x;
    double dy = sy - g_player.y;

    double cs = cos(-g_player.angle);
    double sn = sin(-g_player.angle);
    double tx = dx * cs - dy * sn;
    double ty = dx * sn + dy * cs;
    /* tx = depth (forward), ty = lateral */

    if (tx <= 0.1) return;

    double planeHalf = tan(FOV / 2.0);
    double screenX = (SCREEN_W / 2.0) * (1.0 - ty / (tx * planeHalf));

    int spriteH = (int)((SCREEN_H / tx) * scale);
    int spriteW = spriteH;
    int drawStartY = -spriteH / 2 + SCREEN_H / 2;
    int drawEndY   =  spriteH / 2 + SCREEN_H / 2;
    int drawStartX = (int)(screenX - spriteW / 2);
    int drawEndX   = (int)(screenX + spriteW / 2);

    int sy0 = drawStartY < 0 ? 0 : drawStartY;
    int sy1 = drawEndY >= SCREEN_H ? SCREEN_H - 1 : drawEndY;
    int sx0 = drawStartX < 0 ? 0 : drawStartX;
    int sx1 = drawEndX >= SCREEN_W ? SCREEN_W - 1 : drawEndX;

    int r0 = (tint >> 16) & 0xFF;
    int g0 = (tint >> 8)  & 0xFF;
    int b0 =  tint        & 0xFF;

    for (int x = sx0; x <= sx1; x++) {
        if (tx >= g_depth[x]) continue;
        double u = (x - drawStartX) / (double)spriteW;
        for (int y = sy0; y <= sy1; y++) {
            double v = (y - drawStartY) / (double)spriteH;
            /* Crude imp-like silhouette: body oval + head circle + arms */
            double cx = u - 0.5;
            double cy = v - 0.5;
            int draw = 0;
            uint32_t col = 0;
            /* head */
            if (cx*cx + (cy + 0.30)*(cy + 0.30) < 0.04) { draw = 1; col = 0x804020; }
            /* eyes */
            if ((cx + 0.06)*(cx + 0.06) + (cy + 0.32)*(cy + 0.32) < 0.0012) { draw = 1; col = 0xFFFF00; }
            if ((cx - 0.06)*(cx - 0.06) + (cy + 0.32)*(cy + 0.32) < 0.0012) { draw = 1; col = 0xFFFF00; }
            /* body */
            if ((cx*cx) / 0.04 + ((cy - 0.05) * (cy - 0.05)) / 0.10 < 1.0 && !draw) {
                draw = 1; col = (r0 << 16) | (g0 << 8) | b0;
            }
            /* feet */
            if (cy > 0.32 && cy < 0.45 && fabs(cx) < 0.18 && !draw) {
                draw = 1; col = 0x402010;
            }
            if (draw) {
                /* darken by distance */
                double t = tx / MAX_DEPTH;
                if (t > 1.0) t = 1.0;
                double b = 1.0 - t * 0.7;
                int rr = (int)(((col >> 16) & 0xFF) * b);
                int gg = (int)(((col >> 8)  & 0xFF) * b);
                int bb = (int)(( col        & 0xFF) * b);
                if (g_enemy.hitFlash > 0) {
                    rr = 255; gg = 255; bb = 255;
                }
                putPixel(x, y, (rr << 16) | (gg << 8) | bb);
            }
        }
    }
}

static void drawWeapon(void)
{
    int gx = SCREEN_W / 2;
    int gy = SCREEN_H;
    /* gun body */
    fillRect(gx - 40, gy - 70, 80, 70, 0x303030);
    fillRect(gx - 35, gy - 65, 70, 60, 0x505050);
    fillRect(gx - 8,  gy - 110, 16, 50, 0x202020);  /* barrel */
    fillRect(gx - 4,  gy - 115, 8, 8, 0x101010);    /* muzzle */
    /* sight */
    fillRect(gx - 1, gy - 70, 2, 6, 0xC0C0C0);

    if (g_muzzleFlash > 0) {
        for (int y = -40; y < 10; y++) {
            for (int x = -30; x < 30; x++) {
                if (x*x + y*y < 400) {
                    int px = gx + x;
                    int py = gy - 115 + y;
                    if ((unsigned)px < SCREEN_W && (unsigned)py < SCREEN_H) {
                        int d = (int)sqrt((double)(x*x + y*y));
                        int v = 255 - d * 12;
                        if (v < 0) v = 0;
                        uint32_t c = (v << 16) | (v << 8) | (v / 4);
                        g_pixels[py * SCREEN_W + px] = c;
                    }
                }
            }
        }
    }
}

static void drawDigit(int d, int x, int y, uint32_t c)
{
    static const uint8_t glyph[10][5] = {
        {0x7,0x5,0x5,0x5,0x7}, {0x2,0x6,0x2,0x2,0x7}, {0x7,0x1,0x7,0x4,0x7},
        {0x7,0x1,0x7,0x1,0x7}, {0x5,0x5,0x7,0x1,0x1}, {0x7,0x4,0x7,0x1,0x7},
        {0x7,0x4,0x7,0x5,0x7}, {0x7,0x1,0x1,0x1,0x1}, {0x7,0x5,0x7,0x5,0x7},
        {0x7,0x5,0x7,0x1,0x7},
    };
    if (d < 0 || d > 9) return;
    for (int ry = 0; ry < 5; ry++) {
        for (int rx = 0; rx < 3; rx++) {
            if (glyph[d][ry] & (1 << (2 - rx))) {
                fillRect(x + rx * 3, y + ry * 3, 3, 3, c);
            }
        }
    }
}

static void drawNumber(int n, int x, int y, uint32_t c)
{
    if (n < 0) n = 0;
    char buf[16];
    sprintf(buf, "%d", n);
    int len = (int)strlen(buf);
    for (int i = 0; i < len; i++) {
        drawDigit(buf[i] - '0', x + i * 12, y, c);
    }
}

static void drawHUD(void)
{
    /* HUD strip */
    fillRect(0, SCREEN_H - 40, SCREEN_W, 40, 0x202020);
    fillRect(0, SCREEN_H - 40, SCREEN_W, 2, 0x808080);
    /* Health */
    uint32_t hc = g_player.health > 50 ? 0x00C000
               : g_player.health > 20 ? 0xC0C000 : 0xC02020;
    drawNumber(g_player.health, 20, SCREEN_H - 32, hc);
    /* Ammo */
    drawNumber(g_player.ammo, SCREEN_W - 60, SCREEN_H - 32, 0xC0C040);
}

static void drawCrosshair(void)
{
    int cx = SCREEN_W / 2, cy = SCREEN_H / 2;
    for (int i = -5; i <= 5; i++) {
        if (i == 0) continue;
        putPixel(cx + i, cy, 0xFFFFFF);
        putPixel(cx, cy + i, 0xFFFFFF);
    }
}

static void drawMessage(const char *msg, int x, int y, uint32_t c)
{
    /* very rough: just colored block per char for a banner */
    int len = (int)strlen(msg);
    fillRect(x - 4, y - 4, len * 8 + 8, 16, 0x000000);
    fillRect(x - 4, y - 4, len * 8 + 8, 2, c);
    fillRect(x - 4, y + 10, len * 8 + 8, 2, c);
}

static void tryMove(double nx, double ny)
{
    double pad = 0.15;
    if (!mapCell((int)(nx + pad), (int)g_player.y) &&
        !mapCell((int)(nx - pad), (int)g_player.y))
        g_player.x = nx;
    if (!mapCell((int)g_player.x, (int)(ny + pad)) &&
        !mapCell((int)g_player.x, (int)(ny - pad)))
        g_player.y = ny;
}

static void updateEnemy(double dt)
{
    if (!g_enemy.alive) return;
    if (g_enemy.hitFlash > 0) g_enemy.hitFlash -= dt;
    double dx = g_player.x - g_enemy.x;
    double dy = g_player.y - g_enemy.y;
    double dist = sqrt(dx*dx + dy*dy);
    if (dist > 0.001) {
        dx /= dist; dy /= dist;
        double speed = 0.8 * dt;
        if (dist > 0.8) {
            double nx = g_enemy.x + dx * speed;
            double ny = g_enemy.y + dy * speed;
            if (!mapCell((int)nx, (int)g_enemy.y)) g_enemy.x = nx;
            if (!mapCell((int)g_enemy.x, (int)ny)) g_enemy.y = ny;
        } else {
            /* Attack */
            static double atkCool = 0;
            atkCool -= dt;
            if (atkCool <= 0) {
                g_player.health -= 8;
                atkCool = 1.2;
                if (g_player.health < 0) g_player.health = 0;
            }
        }
    }
}

static void shoot(void)
{
    if (g_player.ammo <= 0) return;
    g_player.ammo--;
    g_muzzleFlash = 4;
    if (!g_enemy.alive) return;

    /* Cast a ray straight forward; if enemy is within a small angular
       window and closer than the wall in that direction, hit it. */
    double dx = g_enemy.x - g_player.x;
    double dy = g_enemy.y - g_player.y;
    double dist = sqrt(dx*dx + dy*dy);
    double angToEnemy = atan2(dy, dx);
    double rel = angToEnemy - g_player.angle;
    while (rel > M_PI) rel -= 2 * M_PI;
    while (rel < -M_PI) rel += 2 * M_PI;
    if (fabs(rel) > 0.08) return;

    /* Check wall distance in player's facing direction */
    double rx = cos(g_player.angle), ry = sin(g_player.angle);
    double t = 0;
    while (t < dist && t < MAX_DEPTH) {
        t += 0.05;
        if (mapCell((int)(g_player.x + rx * t), (int)(g_player.y + ry * t))) {
            return; /* wall blocks shot */
        }
    }
    g_enemy.hitFlash = 0.15;
    g_enemy.alive = 0;
}

static void resetGame(void)
{
    g_player.x = 2.5;
    g_player.y = 2.5;
    g_player.angle = 0;
    g_player.health = 100;
    g_player.ammo = 50;
    g_enemy.x = 12.5;
    g_enemy.y = 12.5;
    g_enemy.alive = 1;
    g_enemy.hitFlash = 0;
}

static void updateGame(double dt)
{
    double moveSpeed = 2.5 * dt;
    double turnSpeed = 2.0 * dt;
    double fx = cos(g_player.angle), fy = sin(g_player.angle);
    double sxv = -sin(g_player.angle), syv = cos(g_player.angle);

    if (g_keys['W'] || g_keys[VK_UP])    tryMove(g_player.x + fx * moveSpeed, g_player.y + fy * moveSpeed);
    if (g_keys['S'] || g_keys[VK_DOWN])  tryMove(g_player.x - fx * moveSpeed, g_player.y - fy * moveSpeed);
    if (g_keys['A'])                     tryMove(g_player.x + sxv * moveSpeed, g_player.y + syv * moveSpeed);
    if (g_keys['D'])                     tryMove(g_player.x - sxv * moveSpeed, g_player.y - syv * moveSpeed);
    if (g_keys[VK_LEFT])                 g_player.angle -= turnSpeed;
    if (g_keys[VK_RIGHT])                g_player.angle += turnSpeed;

    if (g_keyEdge[VK_SPACE]) {
        shoot();
        g_keyEdge[VK_SPACE] = 0;
    }
    if (g_keyEdge['R'] && g_player.health <= 0) {
        resetGame();
        g_keyEdge['R'] = 0;
    }
    if (g_keyEdge[VK_ESCAPE]) {
        g_running = 0;
    }

    if (g_muzzleFlash > 0) g_muzzleFlash--;
    if (g_player.health > 0) updateEnemy(dt);
}

static void renderFrame(void)
{
    for (int x = 0; x < SCREEN_W; x++) castColumn(x);
    if (g_enemy.alive || g_enemy.hitFlash > 0)
        drawSprite(g_enemy.x, g_enemy.y, 1.0, 0xA02020);
    drawCrosshair();
    drawWeapon();
    drawHUD();
    if (g_player.health <= 0) {
        /* red tint */
        for (int i = 0; i < SCREEN_W * SCREEN_H; i++) {
            uint32_t c = g_pixels[i];
            int r = (c >> 16) & 0xFF;
            int g = (c >> 8) & 0xFF;
            int b = c & 0xFF;
            r = (r + 200) / 2; g /= 3; b /= 3;
            g_pixels[i] = (r << 16) | (g << 8) | b;
        }
        drawMessage("YOU DIED - PRESS R", SCREEN_W / 2 - 80, SCREEN_H / 2, 0xFF4040);
    } else if (!g_enemy.alive) {
        drawMessage("ENEMY DOWN", SCREEN_W / 2 - 40, 20, 0x40FF40);
    }
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
        if (wp < 256 && !g_keys[wp]) g_keyEdge[wp] = 1;
        if (wp < 256) g_keys[wp] = 1;
        return 0;
    }
    case WM_KEYUP:
        if (wp < 256) g_keys[wp] = 0;
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
    g_bmi.bmiHeader.biHeight = -SCREEN_H;  /* top-down */
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

    QueryPerformanceFrequency(&g_freq);
    LARGE_INTEGER prev; QueryPerformanceCounter(&prev);

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
        double dt = (now.QuadPart - prev.QuadPart) / (double)g_freq.QuadPart;
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
