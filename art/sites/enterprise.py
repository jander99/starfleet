"""Render the Galaxy-class Enterprise, bow left, at native pixel resolution."""

from math import cos, sin, tau
from pathlib import Path

from PIL import Image, ImageDraw, ImageFilter


OUTLINE = '#05070d'
SHADOW = '#1a2236'
DARK = '#2a3756'
MID = '#44557a'
STEEL = '#6b7fa6'
LIGHT = '#9fb0d8'
PALE = '#cfdaf0'
WHITE = '#eef3ff'
TEAL = '#62c9d9'
BLUE = '#2f8a99'
RED = '#ff7a70'
RED_DARK = '#a83a3a'
DEEP_BLUE = '#23426b'

image = Image.new('RGBA', (160, 80))
draw = ImageDraw.Draw(image)


def polygon(points, fill):
    draw.polygon(points, fill=fill)


def line(points, fill, width=1):
    draw.line(points, fill=fill, width=width)


def saucer_ring(scale=1, dy=0):
    # Intrinsic ellipse: beam 1.6 times fore/aft depth. Its projected
    # cross-ship axis recedes diagonally, exposing the broad dorsal face.
    return [
        (round(46 + scale * (37 * cos(t) + 22 * sin(t))),
         round(29 + dy + scale * (-19 * cos(t) + 13 * sin(t))))
        for t in (tau * n / 120 for n in range(120))
    ]


def nacelle(x, y):
    # Bussard at left, tapered tail at right; two separate flat light planes.
    polygon([(x, y+3), (x+5, y), (x+14, y), (x+58, y+9),
             (x+61, y+12), (x+55, y+15), (x+9, y+8), (x+1, y+7)], DARK)
    polygon([(x+8, y+1), (x+15, y+1), (x+56, y+10),
             (x+49, y+11), (x+9, y+5)], LIGHT)
    line([(x+15, y+2), (x+55, y+10)], PALE)
    polygon([(x+11, y+6), (x+54, y+12), (x+49, y+14), (x+11, y+8)], BLUE)
    line([(x+14, y+6), (x+51, y+12)], TEAL)
    for offset in range(20, 49, 6):
        line([(x+offset, y+7+(offset-14)//7),
              (x+offset, y+9+(offset-14)//7)], DEEP_BLUE)
    polygon([(x+1, y+3), (x+5, y+1), (x+9, y+2),
             (x+10, y+6), (x+6, y+7), (x+1, y+5)], RED_DARK)
    line([(x+2, y+3), (x+5, y+2), (x+8, y+3), (x+8, y+4)], RED)
    line([(x+11, y+2), (x+11, y+5)], STEEL)
    line([(x+55, y+11), (x+58, y+11)], STEEL)


# Far engine and the short, swept attachment, behind the engineering hull.
nacelle(88, 3)
polygon([(96, 35), (105, 33), (119, 19), (114, 17), (102, 25)], MID)
line([(102, 31), (115, 19)], LIGHT, 2)
line([(105, 32), (119, 20)], DARK)

# Long, sloping engineering hull: wide forward belly, narrow aft boat-tail.
polygon([(60, 39), (73, 33), (91, 35), (112, 43), (133, 52),
         (128, 56), (105, 57), (80, 57), (65, 53), (58, 47)], DARK)
polygon([(64, 39), (78, 34), (91, 37), (113, 45), (131, 52),
         (111, 51), (88, 48), (68, 46)], LIGHT)
polygon([(69, 46), (88, 48), (111, 51), (129, 53), (109, 55),
         (81, 54), (65, 50)], STEEL)
line([(89, 39), (112, 47), (125, 51)], PALE)
line([(83, 55), (105, 56), (126, 54)], MID)
line([(95, 47), (111, 51)], MID)
for x, y in [(91, 51), (96, 52), (102, 53), (117, 52)]:
    line([(x, y), (x+1, y)], PALE)

# The characteristic recessed blue navigational deflector, facing the bow.
polygon([(61, 43), (68, 42), (77, 46), (79, 50), (73, 54),
         (65, 53), (59, 48)], SHADOW)
polygon([(62, 44), (68, 44), (75, 47), (76, 50),
         (71, 52), (65, 51), (61, 48)], DEEP_BLUE)
polygon([(63, 45), (68, 45), (73, 47), (74, 49),
         (70, 51), (65, 50), (62, 47)], BLUE)
line([(64, 46), (68, 46), (72, 48), (69, 49), (65, 48)], TEAL)
line([(65, 46), (68, 47)], PALE)

# Broad dorsal neck, mostly hidden under the saucer's aft edge.
polygon([(65, 27), (78, 27), (89, 34), (100, 43),
         (90, 44), (74, 37), (62, 35)], STEEL)
polygon([(69, 28), (78, 28), (91, 38), (85, 38), (70, 32)], PALE)
line([(80, 34), (88, 39)], LIGHT)

# Short near-side pylon bends down and back, not a tall Constitution strut.
polygon([(101, 51), (109, 51), (123, 63), (115, 65), (105, 58)], DARK)
polygon([(104, 51), (109, 52), (120, 62), (115, 62), (107, 56)], LIGHT)
line([(108, 55), (117, 62)], STEEL)
nacelle(94, 61)

# Saucer lower rim and its thin, stepped silhouette.
polygon(saucer_ring(dy=4), DARK)
polygon(saucer_ring(dy=2), STEEL)
polygon(saucer_ring(), LIGHT)
line(saucer_ring()+[saucer_ring()[0]], PALE)
# Shadow on the lower-left lip, upper-right illumination on the dorsal face.
arc = saucer_ring()
line(arc[48:106], STEEL)
line([(x, y+3) for x, y in arc[48:105]], MID)
line(arc[0:43], WHITE)
line(saucer_ring(.86)+[saucer_ring(.86)[0]], STEEL)
line(saucer_ring(.78)+[saucer_ring(.78)[0]], PALE)

# Deliberate radial panel seams, not noise or dither.
for t in [0.15, .72, 1.3, 2.0, 2.65, 3.35, 4.0, 4.65, 5.3, 5.85]:
    def at(s):
        return (round(46+s*(37*cos(t)+22*sin(t))),
                round(29+s*(-19*cos(t)+13*sin(t))))
    line([at(.84), at(.97)], STEEL)

# Raised central bridge island and aft impulse housing.
polygon([(39, 24), (48, 20), (57, 21), (65, 26), (60, 30),
         (49, 31), (40, 28)], STEEL)
polygon([(40, 24), (49, 21), (56, 22), (62, 25), (56, 28),
         (47, 28)], PALE)
draw.ellipse((44, 22, 54, 26), fill=LIGHT)
line([(47, 22), (51, 22), (53, 23)], WHITE)
line([(63, 26), (69, 29)], DARK, 2)
line([(65, 27), (68, 29)], RED_DARK)
draw.point((66, 27), fill=RED)

# Tiny, intentional registry strokes at sprite scale (not blurred text).
line([(24, 28), (30, 25)], MID)
line([(25, 30), (27, 29)], MID)
line([(29, 28), (31, 27)], MID)
line([(33, 26), (35, 25)], MID)
for x, y in [(15, 38), (20, 42), (28, 46), (38, 48), (50, 46),
             (62, 41), (73, 34), (79, 27)]:
    line([(x, y), (x+1, y)], DARK)
draw.point((15, 19), fill=RED)
draw.point((77, 22), fill=TEAL)

# A one-pixel, eight-connected dark outline outside the complete silhouette.
mask = image.getchannel('A')
border = mask.filter(ImageFilter.MaxFilter(3))
outlined = Image.new('RGBA', image.size, OUTLINE)
outlined.putalpha(border)
outlined.alpha_composite(image)
destination = Path(__file__).with_suffix('.png')
outlined.save(destination)

assert outlined.size == (160, 80)
assert not any(outlined.getchannel('A').histogram()[1:255])
assert len(outlined.getcolors()) <= 17  # Shared palette plus transparency.
print(f'Saved {destination}')
