"""Terminal-window chrome for Null's README cards.

The same window as the profile's cards (`dominionism/dominionism`, scripts/terminal.py), so the
README's sections read as one family: dark GitHub window, traffic lights, monospace body.

Every animation is plain CSS with `both` fill, and each element's resting style is its final frame.
So anything that ignores animation (reduced-motion users, static renderers) shows the finished
window instead of a blank one.
"""
from html import escape

W = 860
PAD = 24
BAR = 30                     # title bar height
FS = 13                      # prompt font size
CW = FS * 0.6                # monospace advance at FS
FONT = "ui-monospace,SFMono-Regular,'SF Mono',Menlo,Consolas,'Liberation Mono',monospace"

BG, BAR_BG, BORDER = "#0D1117", "#161B22", "#21262D"
FG, TEXT, MUTED = "#E6EDF3", "#C9D1D9", "#8B949E"
GREEN, BLUE = "#3FB950", "#58A6FF"

USER = "dominionism"
HOST = "null"
PREFIX = f"{USER}@{HOST} ~ $ "

BASE_CSS = f"""
    text {{ font-family: {FONT}; }}
    .rise {{ animation: rise .35s ease-out both; }}
    .fade {{ animation: fade .4s ease-out both; }}
    .blink {{ animation: blink 1.1s steps(1) infinite; }}
    @keyframes rise {{ from {{ opacity: 0; transform: translateY(4px); }} }}
    @keyframes fade {{ from {{ opacity: 0; }} }}
    @keyframes blink {{ 50% {{ opacity: 0; }} }}
    @keyframes hold {{ 0%, 99.9% {{ opacity: 1; }} 100% {{ opacity: 0; }} }}
    @media (prefers-reduced-motion: reduce) {{ * {{ animation: none !important; }} }}"""


def delay(t):
    return f'style="animation-delay:{t:.3f}s"'


def prefix(x, y):
    """The coloured `user@null ~ $` part of a prompt, each piece pinned to its CW column.

    Separate plain texts rather than tspans: WebKit ignores textLength on a text with tspans.
    """
    return "".join(
        f'<text x="{x + PREFIX.index(s) * CW:.1f}" y="{y}" font-size="{FS}" fill="{color}"'
        + (f' textLength="{len(s) * CW:.1f}" lengthAdjust="spacing"' if len(s) > 1 else "")
        + f">{s}</text>"
        for s, color in ((f"{USER}@{HOST}", GREEN), ("~", BLUE), ("$", MUTED))
    )


def prompt(y, cmd, start, name, x=PAD, per_char=0.055):
    """A `user@null ~ $ cmd` line whose command types itself out from `start`.

    Returns (svg, css, t_enter): t_enter is when the command "runs" and output may begin.
    Every glyph sits on the CW grid (textLength), so the typing cover lines up in any font.
    """
    xc = x + len(PREFIX) * CW
    n = len(cmd)
    t_enter = start + n * per_char + 0.25
    top = y - FS
    svg = (
        f'  {prefix(x, y)}\n'
        f'  <text x="{xc:.1f}" y="{y}" font-size="{FS}" fill="{FG}" textLength="{n * CW:.1f}" '
        f'lengthAdjust="spacing">{escape(cmd)}</text>\n'
        f'  <clipPath id="{name}-clip"><rect x="{xc:.1f}" y="{top}" width="{(n + 1) * CW:.1f}" height="{FS + 5}"/></clipPath>\n'
        f'  <g clip-path="url(#{name}-clip)"><g class="{name}">\n'
        f'    <rect x="{xc:.1f}" y="{top}" width="{n * CW:.1f}" height="{FS + 5}" fill="{BG}"/>\n'
        f'    <rect x="{xc:.1f}" y="{top + 1}" width="{CW:.1f}" height="{FS + 3}" fill="{FG}" opacity=".85"/>\n'
        f'  </g></g>\n'
    )
    css = (
        f"\n    .{name} {{ opacity: 0; transform: translateX({n * CW:.1f}px); "
        f"animation: {name} {n * per_char:.3f}s steps({n}, end) {start:.3f}s both, hold {t_enter:.3f}s linear both; }}"
        f"\n    @keyframes {name} {{ from {{ transform: translateX(0); }} }}"
    )
    return svg, css, t_enter


def idle_prompt(y, start, x=PAD):
    """A fresh prompt with a blinking block cursor, shown once the output has printed."""
    xc = x + len(PREFIX) * CW
    return (
        f'  <g class="fade" {delay(start)}>\n'
        f'    {prefix(x, y)}\n'
        f'    <rect class="blink" x="{xc:.1f}" y="{y - FS + 1}" width="{CW:.1f}" height="{FS + 3}" fill="{FG}" opacity=".85"/>\n'
        f'  </g>\n'
    )


def window(h, title, body, css, label, heading=False):
    """Wrap `body` in a dark, rounded terminal window with a title bar.

    With heading=True the title is the section's header, so it reads brighter and bolder.
    """
    title_style = f'font-size="13" font-weight="600" fill="{FG}"' if heading else f'font-size="12" fill="{MUTED}"'
    return (
        f'<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 {W} {h}" width="{W}" height="{h}" '
        f'fill="none" role="img" aria-label="{escape(label)}">\n'
        f'  <title>{escape(label)}</title>\n'
        f'  <style>{BASE_CSS}{css}\n  </style>\n'
        f'  <clipPath id="win"><rect x="0.5" y="0.5" width="{W - 1}" height="{h - 1}" rx="12"/></clipPath>\n'
        f'  <g clip-path="url(#win)">\n'
        f'  <rect width="{W}" height="{h}" fill="{BG}"/>\n'
        f'  <rect width="{W}" height="{BAR}" fill="{BAR_BG}"/>\n'
        f'  <path d="M0 {BAR - 0.5}H{W}" stroke="{BORDER}"/>\n'
        f'  <circle cx="20" cy="{BAR / 2}" r="5.5" fill="#FF5F57"/>\n'
        f'  <circle cx="38" cy="{BAR / 2}" r="5.5" fill="#FEBC2E"/>\n'
        f'  <circle cx="56" cy="{BAR / 2}" r="5.5" fill="#28C840"/>\n'
        f'  <text x="{W / 2}" y="{BAR / 2 + 4}" {title_style} text-anchor="middle">{escape(title)}</text>\n'
        f"{body}"
        f"  </g>\n"
        f'  <rect x="0.5" y="0.5" width="{W - 1}" height="{h - 1}" rx="12" stroke="{BORDER}"/>\n'
        f"</svg>\n"
    )
