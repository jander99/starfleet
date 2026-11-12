"""Draw Terok Nor at native resolution. Run: uv run --with pillow python art/sites/ds9.py."""

from math import cos, sin, pi
from pathlib import Path

from PIL import Image, ImageDraw, ImageFilter


SIZE = 112
INK = "#05070d"
SHADOW = "#1a2236"
DARK = "#34302e"
HULL = "#57504a"
WARM = "#797064"
GREY = "#44557a"
LIGHT = "#9fb0d8"
PALE = "#cfdaf0"
AMBER = "#f2a65a"
RUST = "#b8692c"
image = Image.new("RGBA", (SIZE, SIZE))
draw = ImageDraw.Draw(image)


def project(x, y, z=0):
    return round(57.5 + x), round(64 + y * 0.84 - z * 0.45)


def radial(radius, angle, z=0):
    return project(radius * cos(angle), radius * sin(angle), z)


def stroke(points, color, width=1):
    draw.line(points, fill=color, width=width, joint="curve")


def ring(radius, width, z=0):
    # Each short polygon is one solid, flat-shaded piece, never a gradient.
    for i in range(96):
        a, b = 2 * pi * i / 96, 2 * pi * (i + 1) / 96
        lighting = cos(a) - sin(a)
        color = WARM if lighting > 0.5 else HULL if lighting > -0.5 else DARK
        draw.polygon(
            [radial(radius + width / 2, a, z), radial(radius + width / 2, b, z),
             radial(radius - width / 2, b, z), radial(radius - width / 2, a, z)],
            fill=color,
        )
    for r in (radius - width / 2, radius + width / 2):
        stroke([radial(r, i * pi / 96, z) for i in range(193)], INK)
    stroke([radial(radius + width / 2 - 1, i * pi / 96, z)
            for i in range(104, 191)], LIGHT)


angles = [-pi / 2 + i * 2 * pi / 3 for i in range(3)]

# Underside ledges make the small viewing tilt legible.
ring(42, 8, -4)
for angle in angles:
    stroke([radial(6, angle, -3), radial(43, angle, -3)], INK, 8)
    stroke([radial(7, angle, -2), radial(43, angle, -2)], SHADOW, 6)

# The three spoke assemblies continue through the habitat to the docking ring.
for angle in angles:
    points = [radial(7, angle), radial(43, angle)]
    stroke(points, INK, 7)
    stroke(points, HULL, 5)
    stroke([radial(8, angle, 2), radial(42, angle, 2)], WARM, 2)
    for radius in (15, 30, 35):
        x, y = radius * cos(angle), radius * sin(angle)
        dx, dy = -sin(angle) * 2, cos(angle) * 2
        stroke([project(x - dx, y - dy), project(x + dx, y + dy)], DARK)

ring(23, 5, 2)
ring(42, 7)

# Radial ribs, docking berths and sparse, precisely placed amber windows.
for i in range(36):
    a = i * 2 * pi / 36
    stroke([radial(39, a), radial(45, a)], DARK)
    if i % 3 == 1:
        stroke([radial(40, a + 0.025), radial(43, a + 0.025)], LIGHT)
    if i % 3 != 0:
        draw.point(radial(42, a + 0.06), fill=AMBER if i % 2 else RUST)
for i in range(18):
    a = (i + 0.3) * 2 * pi / 18
    stroke([radial(21, a, 2), radial(25, a, 2)], DARK)
    if i % 2 == 0:
        draw.point(radial(23, a + 0.07, 2), fill=AMBER)
for i in range(9):
    a = (i + 0.5) * 2 * pi / 9
    x, y = radial(46, a)
    draw.rectangle((x - 1, y - 1, x + 1, y + 1), fill=INK)
    draw.point((x, y), fill=WARM)

# Faceted central fusion core, raised ops crown and short antenna.
def hub(radius, z, color):
    draw.polygon([radial(radius, -pi / 2 + i * pi / 6, z)
                  for i in range(12)], fill=color, outline=INK)


hub(11, 0, DARK)
hub(10, 4, HULL)
hub(8, 8, WARM)
stroke([radial(8, -pi / 3, 8), radial(8, -pi / 6, 8), radial(8, 0, 8)], LIGHT)
for a in angles:
    stroke([radial(4, a, 9), radial(8, a, 6)], DARK, 2)
hub(5, 12, GREY)
hub(3, 15, LIGHT)
draw.point(radial(2, -pi / 4, 16), fill=PALE)
stroke([project(0, 0, 16), project(0, 0, 23)], INK, 2)
draw.point(project(0, 0, 23), fill=AMBER)
for a in (-0.2, pi - 0.2):
    draw.point(radial(7, a, 7), fill=AMBER)

# Three raised, curved docking pylons. Their splayed feet and tapering horns
# are projected from the same 120-degree geometry as the station's spokes.
for angle in angles:
    left, right, ridge = [], [], []
    for i in range(25):
        t = i / 24
        radius = 42 + 41 * t - 28 * t * t
        z = 29 * t * t
        bend = 0.10 * sin(pi * t)
        a = angle + bend
        half_width = 4.4 * (1 - t) + 0.7 * t
        x, y = radius * cos(a), radius * sin(a)
        nx, ny = -sin(a), cos(a)
        left.append(project(x + nx * half_width, y + ny * half_width, z))
        right.append(project(x - nx * half_width, y - ny * half_width, z))
        ridge.append(project(x, y, z + 2))
    draw.polygon(left + right[::-1], fill=HULL, outline=INK)
    draw.polygon(left + ridge[::-1], fill=DARK)
    stroke(left + right[::-1] + [left[0]], INK)
    stroke(ridge[:-1], WARM, 2)
    bright_edge = right if cos(angle) - sin(angle) > 0 else left
    stroke(bright_edge[3:-2], LIGHT)
    for j in (4, 9, 14, 19):
        stroke([left[j], right[j]], DARK)
    stroke(ridge[-5:-1], GREY, 2)
    draw.point(ridge[-2], fill=AMBER)

# A true one-pixel exterior outline, including the transparent ring apertures.
alpha = image.getchannel("A")
border = alpha.filter(ImageFilter.MaxFilter(3))
outlined = Image.new("RGBA", image.size, INK)
outlined.putalpha(border)
outlined.alpha_composite(image)
output = Path(__file__).with_suffix(".png")
outlined.save(output)

assert outlined.size == (112, 112)
assert set(outlined.getchannel("A").tobytes()) == {0, 255}
assert len(outlined.getcolors()) <= 16
print(f"Saved {output}")
