"""Generate the Daystrom Institute sprite at native pixel resolution."""

from pathlib import Path

from PIL import Image, ImageDraw, ImageFilter


SIZE = (80, 112)
OUTLINE = "#05070d"
INK = "#1a2236"
SHADOW = "#2a3756"
STEEL = "#44557a"
MID = "#6b7fa6"
LIGHT = "#9fb0d8"
SILVER = "#cfdaf0"
WHITE = "#eef3ff"
TEAL = "#62c9d9"
GLASS = "#2f8a99"
BLUE = "#23426b"
AMBER = "#f2a65a"


def generate():
    image = Image.new("RGBA", SIZE)
    d = ImageDraw.Draw(image)

    def poly(points, color):
        d.polygon(points, fill=color)

    def line(points, color, width=1):
        d.line(points, fill=color, width=width)

    # Broad, terraced research podium. Roofs expose the three-quarter view;
    # left walls stay shaded while upper-right surfaces catch the light.
    poly([(3,89), (39,75), (76,87), (76,96), (43,109), (3,96)], INK)
    poly([(3,89), (39,75), (76,87), (43,101)], MID)
    poly([(3,89), (43,101), (43,108), (3,96)], SHADOW)
    poly([(43,101), (76,87), (76,95), (43,108)], STEEL)
    line([(4,89), (43,100), (75,87)], SILVER)
    line([(44,107), (75,95)], LIGHT)

    # Two low lab wings flank the central tower, with inset window ribbons.
    poly([(7,80), (24,73), (39,78), (39,91), (22,98), (7,92)], SHADOW)
    poly([(7,80), (24,73), (39,78), (22,85)], LIGHT)
    poly([(22,85), (39,78), (39,91), (22,98)], STEEL)
    line([(8,80), (22,84), (38,78)], SILVER)
    line([(10,85), (19,88)], BLUE, 3)
    line([(10,85), (19,88)], GLASS)
    line([(25,89), (36,85)], INK, 3)
    line([(25,88), (36,84)], TEAL)
    for x, y in [(12,86), (16,87), (28,87), (33,85)]:
        line([(x,y), (x,y+3)], SHADOW)
    poly([(45,77), (60,72), (73,77), (73,90), (57,97), (45,91)], STEEL)
    poly([(45,77), (60,72), (73,77), (57,84)], SILVER)
    poly([(57,84), (73,77), (73,90), (57,97)], MID)
    line([(58,84), (72,78)], WHITE)
    line([(60,87), (70,83)], INK, 3)
    line([(60,86), (70,82)], TEAL)
    for x, y in [(63,84), (67,82)]:
        line([(x,y), (x,y+4)], STEEL)
    # Roof vents are deliberate clusters rather than texture/noise.
    for x, y in [(15,79), (19,77), (60,77), (64,78)]:
        line([(x,y), (x+4,y+1)], STEEL)

    # Central entrance and its bright canopy.
    poly([(31,91), (43,86), (54,90), (43,95)], SILVER)
    poly([(35,94), (43,97), (43,103), (35,100)], INK)
    poly([(43,97), (50,94), (50,100), (43,103)], BLUE)
    line([(44,98), (48,96), (48,99)], TEAL)
    line([(34,102), (43,105), (52,101)], LIGHT)

    # Slender six-sided tower and solid plinth.
    poly([(27,75), (40,70), (54,75), (54,85), (41,91), (27,85)], SHADOW)
    poly([(41,81), (54,75), (54,85), (41,91)], MID)
    line([(28,79), (41,84), (53,79)], LIGHT)
    front = [(28,29), (40,34), (40,80), (28,75)]
    right = [(40,34), (52,28), (52,75), (40,80)]
    poly(front, BLUE)
    poly(right, GLASS)

    # Native-resolution hexagonal glazing, clipped to the two glass faces.
    lattice = Image.new("RGBA", SIZE)
    ld = ImageDraw.Draw(lattice)
    for col in range(5):
        cx = 28 + col * 6
        for row in range(8):
            cy = 30 + row * 8 + (col % 2) * 4
            ld.line([(cx-4,cy), (cx-2,cy-4), (cx+2,cy-4),
                     (cx+4,cy), (cx+2,cy+4), (cx-2,cy+4),
                     (cx-4,cy)], fill=TEAL, width=1)
    mask = Image.new("L", SIZE)
    md = ImageDraw.Draw(mask)
    md.polygon(front, fill=255)
    md.polygon(right, fill=255)
    lattice.putalpha(Image.composite(lattice.getchannel("A"),
                                    Image.new("L", SIZE), mask))
    image.alpha_composite(lattice)
    d = ImageDraw.Draw(image)

    # Narrow structural mullions leave the hex core unmistakably exposed.
    line([(28,29), (28,75), (40,81)], STEEL, 2)
    line([(40,34), (40,80)], LIGHT)
    line([(52,28), (52,75), (41,81)], SILVER)
    for y in (43, 66):
        line([(28,y), (40,y+5), (52,y-1)], STEEL)
        d.point((51,y-1), fill=WHITE)

    # Upper crown: visible roof, illuminated right fascia, dark left fascia.
    poly([(25,26), (38,20), (55,25), (42,32)], SILVER)
    poly([(25,26), (42,32), (42,36), (25,30)], STEEL)
    poly([(42,32), (55,25), (55,29), (42,36)], LIGHT)
    line([(26,25), (38,20), (54,25)], WHITE)
    line([(44,33), (53,29)], TEAL)

    # Antenna array and compact tilted parabolic sensor dish.
    line([(32,23), (32,4)], LIGHT)
    line([(29,11), (36,13)], MID)
    line([(29,15), (36,17)], MID)
    line([(37,22), (37,9)], SILVER)
    d.point((32,3), fill=TEAL)
    line([(26,24), (26,16)], STEEL)
    line([(47,23), (47,17)], LIGHT, 2)
    poly([(40,11), (44,10), (54,15), (52,18), (47,19), (43,16)], STEEL)
    poly([(40,11), (44,10), (54,15), (49,16), (44,14)], SILVER)
    line([(41,11), (44,10), (54,15)], WHITE)
    line([(47,13), (50,9)], MID)
    d.point((50,8), fill=TEAL)
    d.point((53,26), fill=AMBER)

    # Expand the binary silhouette exactly one pixel. No blur or antialiasing.
    alpha = image.getchannel("A")
    outline = alpha.filter(ImageFilter.MaxFilter(3))
    result = Image.new("RGBA", SIZE, OUTLINE)
    result.putalpha(outline)
    result.alpha_composite(image)
    result.save(Path(__file__).with_suffix(".png"))
    return result


if __name__ == "__main__":
    generate()
