#!/usr/bin/env python3
"""Draw the README's banner: Null's pet, alone, moving as it does beside the box.

    python3 .github/Readme/make_banner.py

The pet is drawn as the page draws it (`Mini/Page/index.html`): a grid of 16 squares, the same parts
under the same names, and every movement one whole square, never a slide. One loop shows what it does
in the app: it rests, it works while a reply is under way, and it hops once when the reply ends.

The loop is the list of moments below, in seconds. Change a moment and rerun; nothing else in the
repository is read. There is no background and no word in the picture: black with a light edge, the
pet shows on a dark page as well as on a light one.
"""
import os
import xml.etree.ElementTree as ET

OUT = os.path.join(os.path.dirname(os.path.abspath(__file__)), "hero.svg")

SIZE = 208                       # the picture's side: 13 points to a square
INK, EDGE, EYES = "#0b0b0d", "#f4f4f7", "#ffffff"

# The pet's parts, square by square.
BODY = "M6 5h4v1h-4zM4 6h8v2h-8zM3 8h10v4h-10zM4 12h8v2h-8zM6 14h4v1h-4z"
FLAG = "M13 4h1v1h-1zM12 5h1v1h-1z"
TAIL = "M3 14h1v1h-1zM2 15h1v1h-1z"
LEFT = "M5 15h2v1h-2z"
RIGHT = "M9 15h2v1h-2z"
OPEN = "M5 9h1v2h-1zM10 9h1v2h-1z"
GLAD = "M5 9h1v1h-1zM4 10h1v1h-1zM6 10h1v1h-1zM10 9h1v1h-1zM9 10h1v1h-1zM11 10h1v1h-1z"

# The loop: at rest, at work, and the hop. The lengths and the rhythm inside each are the page's.
REST = 5.2
STEP = 0.64                      # one step of the walk in place
WORK = 8 * STEP
HOP = 2.2
T = REST + WORK + HOP
WORKING, HOPPING = REST, REST + WORK

UP, UP_TWO, DOWN = "translateY(-1px)", "translateY(-2px)", "translateY(1px)"
RIGHTWARDS, LEFTWARDS = "translateX(1px)", "translateX(-1px)"


def each_step(start, end):
    """The same stretch of every step of the walk."""
    return [(WORKING + n * STEP + start, WORKING + n * STEP + end) for n in range(round(WORK / STEP))]


# name: (property, its value at rest, [(from, until, value meanwhile)])
MOVES = {
    # The whole pet: two hops, a high one and a low one.
    "hop": ("transform", "none", [
        (HOPPING + 0.22, HOPPING + 0.44, UP_TWO),
        (HOPPING + 0.66, HOPPING + 0.88, UP),
    ]),
    # The body sinks a square: slowly at rest, at each step at work.
    "bob": ("transform", "none", [
        (1.56, 2.6, DOWN),
        (4.16, REST, DOWN),
        *[(a, b, DOWN) for a, b in each_step(0.384, STEP)],
    ]),
    "twitch": ("transform", "none", [(3.0, 3.26, UP)]),
    "blink": ("opacity", "1", [(0.9, 1.08, "0"), (3.4, 3.58, "0")]),
    # At work the eyes look to one side, then the other.
    "scan": ("transform", "none", [
        (WORKING + 0.896, WORKING + 1.28, RIGHTWARDS),
        (WORKING + 2.176, WORKING + 2.56, LEFTWARDS),
        (WORKING + 3.456, WORKING + 3.84, RIGHTWARDS),
        (WORKING + 4.736, WORKING + 5.12, LEFTWARDS),
    ]),
    "left-foot": ("transform", "none", [(a, b, UP) for a, b in each_step(STEP / 2, STEP)]),
    "right-foot": ("transform", "none", [(a, b, UP) for a, b in each_step(0, STEP / 2)]),
    # Its eyes are glad for as long as the hop lasts.
    "open-eyes": ("opacity", "1", [(HOPPING, T, "0")]),
    "glad-eyes": ("opacity", "0", [(HOPPING, T, "1")]),
}

# Which part makes which moves.
PARTS = {
    "all": ["hop"],
    "top": ["bob"],
    "flag": ["twitch"],
    "eyes": ["blink", "scan"],
    "left": ["left-foot"],
    "right": ["right-foot"],
    "open": ["open-eyes"],
    "glad": ["glad-eyes"],
}


def keyframes(name):
    """One move as CSS: the value jumps at each moment and holds until the next."""
    prop, rest, moments = MOVES[name]
    stops = {}
    for start, end, value in sorted(moments):
        stops[round(start, 3)] = value
        if end < T:
            stops.setdefault(round(end, 3), rest)
    for start, _, value in moments:      # a move that begins as another ends
        stops[round(start, 3)] = value
    body = " ".join(f"{t / T * 100:.3f}% {{ {prop}: {value}; }}" for t, value in sorted(stops.items()))
    return f"    @keyframes {name} {{ {body} }}\n"


def css():
    rules = "".join(
        f"    .{part} {{ animation: {', '.join(f'{move} {T:.2f}s steps(1) infinite' for move in moves)}; }}\n"
        for part, moves in PARTS.items()
    )
    return (
        "\n"
        "    .glad { opacity: 0; }\n"
        f"{rules}"
        f"{''.join(keyframes(name) for name in MOVES)}"
        "    @media (prefers-reduced-motion: reduce) { * { animation: none !important; } }\n  "
    )


LABEL = "Null's pet"
ABOUT = (
    "A small round creature drawn in black squares with a light edge: two eyes, two feet, an antenna "
    "and a tail. It moves as it does beside the box in the app: it rests, bobbing and blinking; it "
    "walks in place and looks about, as while a reply is under way; and it hops with glad eyes, as "
    "when a reply ends. Then it rests again. Motion is disabled for reduced motion."
)

# The light edge is each part again, a third of a square wider, under all of the black, so that it
# follows whatever moves. The rows shown are 1 to 17: the pet's own, and two above for the hop.
svg = f'''<svg xmlns="http://www.w3.org/2000/svg" width="{SIZE}" height="{SIZE}" viewBox="0 1 16 16" role="img" aria-labelledby="title description" shape-rendering="crispEdges">
  <title id="title">{LABEL}</title>
  <desc id="description">{ABOUT}</desc>
  <style>{css()}</style>
  <g class="all">
    <g fill="{EDGE}" stroke="{EDGE}" stroke-width=".667" stroke-linejoin="miter">
      <g class="top">
        <path d="{BODY}"/>
        <path class="flag" d="{FLAG}"/>
      </g>
      <path d="{TAIL}"/>
      <path class="left" d="{LEFT}"/>
      <path class="right" d="{RIGHT}"/>
    </g>
    <g fill="{INK}">
      <g class="top">
        <path d="{BODY}"/>
        <path class="flag" d="{FLAG}"/>
        <g class="eyes" fill="{EYES}">
          <path class="open" d="{OPEN}"/>
          <path class="glad" d="{GLAD}"/>
        </g>
      </g>
      <path d="{TAIL}"/>
      <path class="left" d="{LEFT}"/>
      <path class="right" d="{RIGHT}"/>
    </g>
  </g>
</svg>
'''

ET.fromstring(svg)   # a banner that is not well-formed XML never reaches the README
with open(OUT, "w") as handle:
    handle.write(svg)
print(f"wrote hero.svg  ({SIZE}x{SIZE}, {len(svg) // 1024} KB, a loop of {T:.2f} s)")
