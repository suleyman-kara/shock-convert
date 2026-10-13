"""Geçici uygulama ikonunu (şimşek) üretir: assets/icon.png (256x256).
Sonra ICO'ya çevirmek için:  cargo run -p shock-convert -- run ico assets/icon.png
Saf Python, ek bağımlılık yok. Gerçek bir logo gelince assets/icon.ico'yu değiştirmek yeterli."""
import struct, sys, zlib

N, SS = 256, 4  # çıktı boyutu, süper örnekleme (kenar yumuşatma)
BOLT = [(0.60, 0.06), (0.24, 0.54), (0.45, 0.54), (0.36, 0.95), (0.78, 0.40), (0.56, 0.40), (0.70, 0.06)]


def inside_poly(x, y, pts):
    c = False
    j = len(pts) - 1
    for i in range(len(pts)):
        xi, yi = pts[i]
        xj, yj = pts[j]
        if (yi > y) != (yj > y) and x < (xj - xi) * (y - yi) / (yj - yi) + xi:
            c = not c
        j = i
    return c


def inside_round_rect(x, y, r):
    # [0,1]² içinde köşe yarıçapı r
    cx = min(max(x, r), 1 - r)
    cy = min(max(y, r), 1 - r)
    return (x - cx) ** 2 + (y - cy) ** 2 <= r * r


def lerp(a, b, t):
    return tuple(a[i] + (b[i] - a[i]) * t for i in range(3))


rows = []
for py in range(N):
    row = bytearray([0])
    for px in range(N):
        acc = [0.0, 0.0, 0.0, 0.0]
        for sy in range(SS):
            for sx in range(SS):
                x = (px + (sx + 0.5) / SS) / N
                y = (py + (sy + 0.5) / SS) / N
                if not inside_round_rect(x, y, 0.22):
                    continue
                col = lerp((27, 31, 77), (78, 52, 200), (x + y) / 2)  # lacivert -> mor/mavi
                if inside_poly(x, y, BOLT):
                    col = lerp((255, 236, 90), (255, 160, 30), y)  # sarı -> turuncu
                acc[0] += col[0]; acc[1] += col[1]; acc[2] += col[2]; acc[3] += 255
        n = SS * SS
        a = acc[3] / n
        if acc[3] == 0:
            row += bytes([0, 0, 0, 0])
        else:
            # önceden çarpılmamış renk: yalnızca kapsanan örneklerin ortalaması
            k = acc[3] / 255
            row += bytes([round(acc[0] / k), round(acc[1] / k), round(acc[2] / k), round(a)])
    rows.append(bytes(row))


def chunk(t, d):
    return struct.pack(">I", len(d)) + t + d + struct.pack(">I", zlib.crc32(t + d))


png = (
    b"\x89PNG\r\n\x1a\n"
    + chunk(b"IHDR", struct.pack(">IIBBBBB", N, N, 8, 6, 0, 0, 0))
    + chunk(b"IDAT", zlib.compress(b"".join(rows), 9))
    + chunk(b"IEND", b"")
)
out = sys.argv[1] if len(sys.argv) > 1 else "assets/icon.png"
open(out, "wb").write(png)
print("yazıldı:", out)
