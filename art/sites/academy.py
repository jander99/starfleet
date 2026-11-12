"""Render the Academy site sprite at native resolution, without antialiasing."""

from pathlib import Path

from PIL import Image, ImageDraw, ImageFilter


SIZE = (120, 80)
INK = "#05070d"
SHADOW = "#1a2236"
DARK = "#2a3756"
SLATE = "#44557a"
STEEL = "#6b7fa6"
LIGHT = "#9fb0d8"
PALE = "#cfdaf0"
WHITE = "#eef3ff"
TEAL = "#62c9d9"
TEAL_DARK = "#2f8a99"
AMBER = "#f2a65a"
AMBER_DARK = "#b8692c"
RED = "#ff7a70"
RED_DARK = "#a83a3a"
BLUE = "#23426b"


def render():
    image = Image.new("RGBA", SIZE)
    draw = ImageDraw.Draw(image)

    # A distant bridge fragment: open tower portals and sparse suspension cables.
    draw.line([(3, 23), (9, 19), (17, 8), (26, 20), (34, 24)], RED_DARK)
    draw.line([(17, 8), (28, 16), (40, 18)], RED)
    for x, top, bottom in [(6, 22, 29), (10, 18, 29), (26, 20, 30), (32, 23, 31)]:
        draw.line([(x, top), (x, bottom)], RED_DARK)
    draw.polygon([(3, 29), (35, 30), (41, 33), (3, 32)], RED_DARK)
    draw.line([(3, 29), (35, 30), (40, 32)], RED)
    draw.rectangle((13, 5, 15, 31), fill=RED_DARK)
    draw.rectangle((20, 4, 22, 32), fill=RED_DARK)
    draw.line((15, 5, 15, 29), fill=RED)
    draw.line((22, 4, 22, 31), fill=RED)
    for y in (8, 15, 24):
        draw.polygon([(13, y), (22, y - 1), (22, y + 1), (13, y + 2)], RED_DARK)
        draw.line([(14, y), (22, y - 1)], RED)

    # Only the architectural forecourt is present, not a surrounding ground tile.
    draw.polygon([(33, 55), (83, 48), (108, 62), (66, 76), (29, 63)], DARK)
    draw.polygon([(33, 53), (83, 46), (108, 60), (66, 73), (29, 61)], STEEL)
    draw.line([(30, 61), (66, 73), (108, 60)], LIGHT)
    draw.line([(35, 61), (66, 71), (101, 60)], SLATE)
    draw.line([(54, 58), (84, 67)], SLATE)
    draw.line([(48, 66), (88, 54)], SLATE)

    # Broad low hall: a dark left elevation and a brighter right return.
    draw.polygon([(12, 36), (38, 28), (100, 32), (111, 41), (110, 52),
                  (72, 63), (16, 49)], SHADOW)
    draw.polygon([(16, 36), (74, 43), (74, 60), (16, 47)], DARK)
    draw.polygon([(74, 43), (110, 34), (110, 51), (74, 61)], STEEL)
    draw.polygon([(19, 38), (70, 44), (70, 56), (19, 45)], BLUE)
    draw.polygon([(77, 44), (107, 36), (107, 48), (77, 57)], BLUE)

    # Window panes are discrete warm pixels, framed by cool glass and mullions.
    for x in range(21, 69, 7):
        y = 39 + (x - 21) // 5
        draw.polygon([(x, y), (x + 4, y + 1), (x + 4, y + 7), (x, y + 6)], AMBER_DARK)
        draw.line([(x + 1, y + 1), (x + 3, y + 1)], AMBER)
        draw.line([(x + 3, y + 2), (x + 3, y + 5)], AMBER)
    for x in range(79, 105, 7):
        y = 44 - (x - 79) // 4
        draw.polygon([(x, y), (x + 4, y - 1), (x + 4, y + 6), (x, y + 7)], AMBER_DARK)
        draw.polygon([(x + 1, y), (x + 4, y - 1), (x + 4, y + 4), (x + 1, y + 5)], AMBER)
    draw.line([(18, 47), (73, 60)], SLATE)
    draw.line([(75, 60), (110, 50)], LIGHT)
    draw.line([(78, 50), (106, 43)], SLATE)

    # Raised entrance pavilion, with glass doors under the curved canopy.
    draw.polygon([(58, 45), (73, 48), (83, 45), (83, 58), (72, 62), (58, 58)], DARK)
    draw.polygon([(61, 48), (72, 50), (72, 59), (61, 56)], TEAL_DARK)
    draw.polygon([(74, 50), (80, 48), (80, 56), (74, 58)], TEAL)
    draw.line([(66, 49), (66, 57)], SLATE)
    draw.line([(77, 50), (77, 56)], PALE)
    draw.polygon([(58, 59), (72, 63), (85, 59), (87, 60), (72, 66), (56, 61)], LIGHT)
    draw.line([(58, 61), (72, 65), (86, 60)], PALE)

    # Sweeping roof, described by hand-stepped curves, never smoothed or resized.
    roof = [(9, 34), (18, 30), (29, 27), (41, 26), (54, 25), (67, 23),
            (79, 19), (89, 14), (97, 10), (100, 12), (98, 17), (100, 21),
            (106, 25), (113, 28), (116, 31), (110, 35), (98, 39),
            (85, 42), (73, 44), (59, 43), (43, 40), (27, 38), (13, 37)]
    draw.polygon(roof, PALE)
    draw.polygon([(9, 34), (27, 35), (43, 37), (59, 40), (73, 41),
                  (85, 39), (98, 36), (110, 32), (116, 31), (110, 35),
                  (98, 39), (85, 42), (73, 44), (59, 43), (43, 40),
                  (27, 38), (13, 37)], SLATE)
    draw.line([(13, 34), (28, 35), (44, 37), (60, 40), (73, 41),
               (85, 39), (98, 36), (110, 32), (114, 31)], LIGHT)
    draw.polygon([(30, 31), (46, 29), (62, 28), (76, 24), (87, 19),
                  (96, 13), (92, 20), (81, 27), (67, 31), (51, 32)], LIGHT)
    draw.line([(48, 27), (64, 25), (79, 21), (89, 16), (97, 11)], WHITE)
    draw.line([(99, 21), (105, 25), (113, 29)], WHITE)
    draw.line([(34, 33), (49, 34), (64, 36), (76, 36), (88, 33)], STEEL)

    # Two compact planters flank the forecourt; no landscape extends beyond it.
    for x, y in ((36, 57), (95, 57)):
        draw.polygon([(x - 3, y), (x + 1, y - 2), (x + 5, y),
                      (x + 5, y + 3), (x + 1, y + 5), (x - 3, y + 3)], DARK)
        draw.polygon([(x - 3, y), (x + 1, y - 2), (x + 5, y), (x + 1, y + 2)], TEAL_DARK)
        draw.line([(x + 1, y - 2), (x + 4, y)], TEAL)

    # Gold delta inlaid into the small plaza, foreshortened with the paving.
    draw.polygon([(67, 64), (58, 69), (65, 68), (70, 71)], SHADOW)
    draw.polygon([(67, 63), (58, 68), (65, 67), (70, 70)], AMBER)
    draw.polygon([(67, 65), (64, 67), (67, 67), (69, 69)], AMBER_DARK)

    # A one-pixel dark contour around all exposed silhouette edges.
    alpha = image.getchannel("A")
    contour = alpha.filter(ImageFilter.MaxFilter(3))
    outlined = Image.new("RGBA", SIZE, INK)
    outlined.putalpha(contour)
    outlined.alpha_composite(image)
    return outlined


if __name__ == "__main__":
    render().save(Path(__file__).with_suffix(".png"))
