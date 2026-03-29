"""
Recoverer — demo GIF generator
Renders fake-but-realistic UI frames with Pillow and compiles them into a looping GIF.
Run: python scripts/gen_demo_gif.py
Output: docs/demo.gif  (~400 KB)
"""
from PIL import Image, ImageDraw, ImageFont
import os, math

# ── Dimensions ───────────────────────────────────────────────────────────────
W, H = 780, 520
NAV_W = 200

# ── Palette (from App.xaml + WinUI 3 dark theme) ─────────────────────────────
BG          = (20,  20,  20)       # window / mica dark
NAV_BG      = (26,  26,  26)
NAV_SEL_BG  = (45,  45,  55)
CARD_BG     = (38,  38,  38)
CARD_BD     = (60,  60,  60)
DIVIDER     = (50,  50,  50)
TEXT        = (232, 232, 232)
TEXT_SEC    = (148, 148, 148)
TEXT_TER    = (90,  90,  90)
ACCENT      = (58,  159, 219)      # #3A9FDB
ACCENT_DIM  = (30,  80,  120)
SUCCESS     = (61,  184, 122)      # #3DB87A
WARNING     = (224, 160, 32)       # #E0A020
ERROR       = (217, 88,  88)       # #D95858
BTN_ACC     = (58,  159, 219)
BTN_ACC_TXT = (255, 255, 255)
BTN_BG      = (55,  55,  55)
BTN_TXT     = (220, 220, 220)
BADGE_BG    = (58,  159, 219)
BADGE_TXT   = (255, 255, 255)
PROG_TRACK  = (55,  55,  55)

# ── Font loader ───────────────────────────────────────────────────────────────
FONT_DIR = "C:/Windows/Fonts/"
_font_cache = {}

def font(size, bold=False):
    key = (size, bold)
    if key in _font_cache:
        return _font_cache[key]
    names = (["segoeuib.ttf", "arialbd.ttf"] if bold
             else ["segoeui.ttf", "arial.ttf", "calibri.ttf"])
    f = None
    for n in names:
        try:
            f = ImageFont.truetype(FONT_DIR + n, size)
            break
        except Exception:
            pass
    if f is None:
        f = ImageFont.load_default()
    _font_cache[key] = f
    return f

# ── Helpers ───────────────────────────────────────────────────────────────────
def card(draw, x, y, w, h, r=8, bg=CARD_BG, bd=CARD_BD):
    draw.rounded_rectangle([x, y, x+w, y+h], radius=r, fill=bg, outline=bd, width=1)

def pill(draw, x, y, text, fg=BADGE_TXT, bg=BADGE_BG, size=10, r=4):
    f = font(size)
    bb = draw.textbbox((0, 0), text, font=f)
    tw, th = bb[2]-bb[0], bb[3]-bb[1]
    px, py = 7, 3
    draw.rounded_rectangle([x, y, x+tw+px*2, y+th+py*2], radius=r, fill=bg)
    draw.text((x+px, y+py), text, font=f, fill=fg)
    return tw + px*2

def label(draw, x, y, text, size=10, fg=TEXT_SEC, bold=False):
    draw.text((x, y), text.upper(), font=font(size, bold=True if bold else False), fill=fg)

def text_line(draw, x, y, text, size=12, fg=TEXT, bold=False):
    draw.text((x, y), text, font=font(size, bold=bold), fill=fg)

def progress_bar(draw, x, y, w, h, pct, r=4, track=PROG_TRACK, fill=ACCENT):
    draw.rounded_rectangle([x, y, x+w, y+h], radius=r, fill=track)
    if pct > 0:
        fw = max(r*2, int(w * pct / 100))
        draw.rounded_rectangle([x, y, x+fw, y+h], radius=r, fill=fill)

def button(draw, x, y, text, w=None, accent=False, size=12):
    f = font(size)
    bb = draw.textbbox((0,0), text, font=f)
    tw = bb[2]-bb[0]
    bw = w if w else tw + 28
    bh = 32
    bg = BTN_ACC if accent else BTN_BG
    fg = BTN_ACC_TXT if accent else BTN_TXT
    draw.rounded_rectangle([x, y, x+bw, y+bh], radius=5, fill=bg)
    tx = x + (bw - tw) // 2
    ty = y + (bh - (bb[3]-bb[1])) // 2
    draw.text((tx, ty), text, font=f, fill=fg)
    return bw

def nav_panel(draw, active="Setup"):
    draw.rectangle([0, 0, NAV_W, H], fill=NAV_BG)
    draw.line([NAV_W, 0, NAV_W, H], fill=DIVIDER, width=1)

    # App title
    draw.text((16, 18), "Recoverer", font=font(15, bold=True), fill=TEXT)

    items = [
        ("Scan Setup",  "Setup"),
        ("Scanning",    "Scanning"),
        ("Results",     "Results"),
        ("Recovery",    "Recovery"),
    ]
    for i, (label_text, tag) in enumerate(items):
        y = 60 + i * 44
        is_active = tag == active
        if is_active:
            draw.rounded_rectangle([6, y-2, NAV_W-6, y+32], radius=6, fill=NAV_SEL_BG)
        draw.text((18, y+4), label_text, font=font(13, bold=is_active),
                  fill=TEXT if is_active else TEXT_SEC)

# ── Content panels ────────────────────────────────────────────────────────────
CX = NAV_W + 16   # content area left
CW = W - CX - 16  # content area width

def draw_setup(draw, expanded=False):
    nav_panel(draw, "Setup")
    x, y = CX, 20

    # Expander
    card(draw, x, y, CW, 38 if not expanded else 178, r=8)
    draw.text((x+14, y+10), "Why Recoverer is different", font=font(12), fill=TEXT)
    # chevron
    cx2 = x + CW - 24
    cy2 = y + 17
    pts = [(cx2-5, cy2-3), (cx2, cy2+3), (cx2+5, cy2-3)] if not expanded else [(cx2-5, cy2+3), (cx2, cy2-3), (cx2+5, cy2+3)]
    draw.polygon(pts, fill=TEXT_SEC)

    if expanded:
        # Show 2 differentiator items
        iy = y + 46
        for title, desc in [
            ("Fragment chain detection", "1,000 video chunks → 1 entry. Full span recovered in one pass."),
            ("Cross-session memory",     "Re-scan same drive — recovered files auto-marked, no duplicates."),
        ]:
            draw.rounded_rectangle([x+10, iy, x+16, iy+36], radius=2, fill=ACCENT)
            draw.text((x+24, iy+2), title, font=font(11, bold=True), fill=TEXT)
            draw.text((x+24, iy+18), desc, font=font(10), fill=TEXT_SEC)
            iy += 50

    y += (180 if expanded else 50)

    # WHERE TO SCAN
    label(draw, x, y, "Where to scan", size=9)
    y += 18
    card(draw, x, y, CW, 56, r=8)
    draw.ellipse([x+14, y+12, x+26, y+24], outline=ACCENT, width=2)
    draw.ellipse([x+18, y+16, x+22, y+20], fill=ACCENT)
    draw.text((x+34, y+10), "Entire Drive", font=font(12, bold=True), fill=TEXT)
    # Drive combo
    card(draw, x+34, y+30, CW-50, 18, r=4, bg=(45,45,45), bd=CARD_BD)
    draw.text((x+42, y+33), "C:\\  (223 GB)", font=font(10), fill=TEXT_SEC)
    y += 68

    # WHAT TO LOOK FOR
    label(draw, x, y, "What to look for", size=9)
    y += 18
    pills = [("Images", True), ("Videos", True), ("Documents", False),
             ("Audio", False),  ("Archives", False), ("Other", False)]
    px = x
    for name, checked in pills:
        bg = ACCENT if checked else (50, 50, 50)
        fg = BTN_ACC_TXT if checked else TEXT_SEC
        bw = pill(draw, px, y, name, fg=fg, bg=bg, size=11, r=14)
        px += bw + 8
        if px > x + CW - 80:
            px = x
            y += 30
    y += 38

    # SCAN DEPTH — just show Deep Scan selected
    label(draw, x, y, "Scan Depth", size=9)
    y += 18
    card(draw, x, y, CW, 48, r=8, bg=(28,42,55), bd=ACCENT_DIM)
    draw.ellipse([x+14, y+14, x+26, y+26], outline=ACCENT, width=2)
    draw.ellipse([x+18, y+18, x+22, y+22], fill=ACCENT)
    draw.text((x+34, y+10), "Deep Scan", font=font(12, bold=True), fill=TEXT)
    draw.text((x+34, y+28), "MFT + raw carve · recommended for older deletions", font=font(10), fill=TEXT_SEC)
    pill(draw, x+CW-100, y+16, "Recommended", size=9, r=4)

    # Start button
    button(draw, x+CW-120, y+70, "  Start Scan  ", accent=True)


def draw_scanning(draw, pct=0, files=0, feed_lines=None):
    nav_panel(draw, "Scanning")
    x, y = CX, 20

    # Phase label
    phase = "Scanning MFT..." if pct < 30 else ("Carving sectors..." if pct < 90 else "Finalising...")
    draw.text((x, y), phase, font=font(18, bold=True), fill=TEXT)
    y += 40

    # Big file count
    draw.text((x, y), str(files), font=font(46, bold=True), fill=ACCENT)
    bb = draw.textbbox((x,y), str(files), font=font(46, bold=True))
    draw.text((x + bb[2]-bb[0] + 10, y + 26), "files found", font=font(14), fill=TEXT_SEC)
    y += 60

    # Progress bar
    progress_bar(draw, x, y, CW - 210, 8, pct)
    draw.text((x, y + 14), f"{pct}%", font=font(10), fill=TEXT_SEC)
    if pct > 0:
        eta = f"~{max(1, int((100-pct)*0.4))} min remaining"
        draw.text((x+60, y+14), eta, font=font(10), fill=TEXT_SEC)
    y += 36

    # Pause / Cancel buttons
    button(draw, x, y, "Pause", w=80)
    button(draw, x+88, y, "Cancel", w=80)
    y += 52

    # Live discovery feed
    label(draw, x, y, "Live Discovery", size=9)
    y += 16
    card(draw, x, y, CW-210, 200, r=8)
    if feed_lines:
        for i, (cat, name) in enumerate(feed_lines[-8:]):
            fy = y + 10 + i*22
            draw.text((x+12, fy), cat, font=font(10), fill=ACCENT)
            draw.text((x+100, fy), name, font=font(10), fill=TEXT_SEC)

    # BY TYPE sidebar
    sx = x + CW - 196
    label(draw, sx, 60, "By Type", size=9)
    card(draw, sx, 78, 190, 200, r=8)
    cats = [("Images", int(files*0.45)), ("Videos", int(files*0.3)),
            ("Documents", int(files*0.12)), ("Audio", int(files*0.08)),
            ("Archives", int(files*0.03)), ("Other", int(files*0.02))]
    for i, (cat, cnt) in enumerate(cats):
        cy2 = 92 + i*30
        draw.text((sx+12, cy2), cat, font=font(11), fill=TEXT)
        draw.text((sx+140, cy2), str(cnt), font=font(11, bold=True), fill=ACCENT)
        if files > 0:
            bar_w = int(170 * cnt / max(1, files))
            progress_bar(draw, sx+12, cy2+16, 170, 4, int(cnt*100/max(1,files)), r=2)


def draw_results(draw, total=1247, selected=3):
    nav_panel(draw, "Results")

    # Sidebar
    sx, sy = NAV_W, 0
    draw.rectangle([sx, sy, sx+190, H], fill=(25,25,25))
    draw.line([sx+190, 0, sx+190, H], fill=DIVIDER)

    label(draw, sx+12, sy+16, "File Type", size=9)
    for i, t in enumerate(["All types", "Images", "Videos", "Documents", "Audio", "Archives"]):
        ty = sy + 34 + i*22
        fg = ACCENT if i == 0 else TEXT_SEC
        draw.text((sx+12, ty), t, font=font(11), fill=fg)

    label(draw, sx+12, sy+178, "Confidence", size=9)
    draw.text((sx+12, sy+196), "Min:  55%", font=font(10), fill=TEXT_SEC)
    progress_bar(draw, sx+12, sy+212, 166, 6, 55, r=3)

    button(draw, sx+12, sy+228, "Select High Confidence", w=166, size=10)

    # Main area
    mx = NAV_W + 192
    mw = W - mx - 8

    # Search bar
    card(draw, mx, 12, mw, 30, r=6, bg=(40,40,40))
    draw.text((mx+10, 20), "Search by filename...", font=font(11), fill=TEXT_TER)

    # Header row
    hy = 52
    for col, txt in [(0, "Name"), (260, "Size"), (320, "Confidence"), (400, "Status")]:
        draw.text((mx+col+4, hy), txt, font=font(10), fill=TEXT_SEC)
    draw.line([mx, hy+18, W-8, hy+18], fill=DIVIDER)

    files = [
        ("VIDEO", "vacation_2023.mp4", "3.8 GB", 4, False, False),
        ("IMAGE", "IMG_4521.jpg",       "4.2 MB", 5, True,  False),
        ("IMAGE", "IMG_4522.jpg",       "4.1 MB", 5, True,  False),
        ("DOC",   "report_final.docx",  "892 KB", 3, True,  False),
        ("VIDEO", "birthday_party… (chain ×12)", "18.4 GB", 4, False, True),
        ("AUDIO", "recording_01.mp3",   "12 MB",  3, False, False),
        ("IMAGE", "screenshot_001.png", "880 KB", 5, False, False),
    ]
    colors = {"VIDEO": (58,159,219), "IMAGE": (61,184,122),
              "DOC": (224,160,32), "AUDIO": (200,100,200), "OTHER": TEXT_SEC}

    for i, (cat, name, size, conf, sel, chain) in enumerate(files):
        ry = hy + 22 + i*34
        if sel:
            draw.rectangle([mx, ry-2, W-8, ry+28], fill=(38,55,72))
        col_c = colors.get(cat, TEXT_SEC)
        draw.rounded_rectangle([mx+2, ry+4, mx+36, ry+22], radius=3, fill=(col_c[0]//3, col_c[1]//3, col_c[2]//3))
        draw.text((mx+6, ry+7), cat[:3], font=font(9, bold=True), fill=col_c)
        draw.text((mx+42, ry+5), name[:36], font=font(11), fill=TEXT)
        draw.text((mx+264, ry+5), size, font=font(10), fill=TEXT_SEC)
        # Confidence dots
        for d in range(5):
            dc = ACCENT if d < conf else CARD_BD
            draw.ellipse([mx+326+d*14, ry+9, mx+334+d*14, ry+17], fill=dc)
        if chain:
            pill(draw, mx+406, ry+6, "chain", size=9, r=3)

    # Action bar
    draw.rectangle([mx-4, H-46, W, H], fill=(30,30,30))
    draw.line([mx-4, H-46, W, H-46], fill=DIVIDER)
    draw.text((mx+4, H-32), f"{selected} of {total} selected", font=font(11), fill=TEXT_SEC)
    button(draw, W-240, H-40, f"Recover All ({total})", w=110, size=10)
    button(draw, W-122, H-40, "Recover Selected", accent=True, w=110, size=10)


def draw_recovery(draw, pct=0, recovered=0, warnings=0, failed=0, complete=False):
    nav_panel(draw, "Recovery")
    x, y = CX, 20

    # Destination
    label(draw, x, y, "Destination Folder", size=9)
    y += 18
    card(draw, x, y, CW, 42, r=8)
    draw.text((x+14, y+12), "D:\\Recovered Files", font=font(12), fill=TEXT)
    button(draw, x+CW-74, y+7, "Browse", w=62, size=11)
    y += 56

    # Summary card
    card(draw, x, y, CW, 38, r=8)
    draw.text((x+14, y+11), "3 files selected for recovery   (22.4 GB total)", font=font(12), fill=TEXT)
    y += 50

    if not complete:
        # Progress
        progress_bar(draw, x, y, CW, 10, pct, r=5)
        y += 24
        draw.text((x, y), f"{pct}%  complete", font=font(11), fill=TEXT_SEC)
        y += 26

        # Stat cards
        sw = (CW - 16) // 3
        for i, (val, lbl, col) in enumerate([
            (recovered, "Recovered",    SUCCESS),
            (warnings,  "Low confidence", WARNING),
            (failed,    "Failed",       ERROR),
        ]):
            cx2 = x + i*(sw+8)
            card(draw, cx2, y, sw, 64, r=8)
            draw.text((cx2+14, y+8), str(val), font=font(28, bold=True), fill=col)
            draw.text((cx2+14, y+44), lbl, font=font(10), fill=TEXT_SEC)
        y += 80

        button(draw, x, y, "Cancel Remaining", w=150)
    else:
        # Complete card
        card(draw, x, y, CW, 80, r=8)
        draw.text((x+18, y+16), str(recovered), font=font(28, bold=True), fill=SUCCESS)
        draw.text((x+18+len(str(recovered))*18, y+28), "files recovered", font=font(14), fill=TEXT)
        draw.text((x+18, y+56), f"{warnings} low confidence", font=font(11), fill=WARNING)
        draw.text((x+140, y+56), f"{failed} failed", font=font(11), fill=ERROR)
        y += 96
        button(draw, x, y, "View in Explorer", w=140)
        button(draw, x+148, y, "Scan Again", w=110)


# ── Frame composer ────────────────────────────────────────────────────────────
def make_frame(page, **kw):
    img = Image.new("RGB", (W, H), BG)
    draw = ImageDraw.Draw(img)
    # Titlebar strip
    draw.rectangle([0, 0, W, 1], fill=(40,40,40))
    if page == "setup":
        draw_setup(draw, **kw)
    elif page == "scanning":
        draw_scanning(draw, **kw)
    elif page == "results":
        draw_results(draw, **kw)
    elif page == "recovery":
        draw_recovery(draw, **kw)
    return img


# ── Scene list ────────────────────────────────────────────────────────────────
FEED_PARTIAL = [
    ("IMAGE", "IMG_4521.jpg"),
    ("VIDEO", "clip_003.mp4"),
    ("IMAGE", "IMG_4522.jpg"),
    ("DOC",   "report_final.docx"),
]
FEED_FULL = FEED_PARTIAL + [
    ("VIDEO", "vacation_2023.mp4"),
    ("AUDIO", "recording_01.mp3"),
    ("IMAGE", "photo_export_14.jpg"),
    ("VIDEO", "birthday_party_raw.mp4"),
]

scenes = [
    # Setup page
    dict(ms=1800, page="setup", expanded=False),
    dict(ms=800,  page="setup", expanded=True),
    dict(ms=1800, page="setup", expanded=True),

    # Scanning — animate progress
    dict(ms=600,  page="scanning", pct=0,   files=0,    feed_lines=[]),
    dict(ms=500,  page="scanning", pct=8,   files=120,  feed_lines=FEED_PARTIAL[:2]),
    dict(ms=500,  page="scanning", pct=22,  files=480,  feed_lines=FEED_PARTIAL),
    dict(ms=500,  page="scanning", pct=41,  files=870,  feed_lines=FEED_FULL[:5]),
    dict(ms=500,  page="scanning", pct=60,  files=1100, feed_lines=FEED_FULL),
    dict(ms=500,  page="scanning", pct=78,  files=1200, feed_lines=FEED_FULL),
    dict(ms=600,  page="scanning", pct=100, files=1247, feed_lines=FEED_FULL),
    dict(ms=800,  page="scanning", pct=100, files=1247, feed_lines=FEED_FULL),

    # Results page
    dict(ms=1800, page="results", total=1247, selected=0),
    dict(ms=800,  page="results", total=1247, selected=3),
    dict(ms=1600, page="results", total=1247, selected=3),

    # Recovery — progress
    dict(ms=600,  page="recovery", pct=0,   recovered=0, warnings=0, failed=0, complete=False),
    dict(ms=500,  page="recovery", pct=25,  recovered=1, warnings=0, failed=0, complete=False),
    dict(ms=500,  page="recovery", pct=66,  recovered=2, warnings=1, failed=0, complete=False),
    dict(ms=600,  page="recovery", pct=100, recovered=3, warnings=1, failed=0, complete=False),
    dict(ms=900,  page="recovery", pct=100, recovered=3, warnings=1, failed=0, complete=True),
    dict(ms=2000, page="recovery", pct=100, recovered=3, warnings=1, failed=0, complete=True),

    # Loop back to setup
    dict(ms=1000, page="setup", expanded=False),
]

# ── Render ────────────────────────────────────────────────────────────────────
print("Rendering frames...")
frames = [make_frame(**{k: v for k, v in s.items() if k != "ms"}) for s in scenes]
durations = [s["ms"] for s in scenes]

print("Quantizing...")
quant = [f.quantize(colors=128, method=Image.Quantize.MEDIANCUT) for f in frames]

out_path = os.path.join(os.path.dirname(__file__), "..", "docs", "demo.gif")
os.makedirs(os.path.dirname(out_path), exist_ok=True)

print(f"Saving to {out_path} ...")
quant[0].save(
    out_path, save_all=True,
    append_images=quant[1:],
    duration=durations,
    loop=0,
    optimize=True,
)

size_kb = os.path.getsize(out_path) / 1024
print(f"Done — {size_kb:.0f} KB")

if size_kb > 2000:
    print("Over 2 MB, re-quantizing to 64 colors...")
    quant64 = [f.quantize(colors=64, method=Image.Quantize.MEDIANCUT) for f in frames]
    quant64[0].save(
        out_path, save_all=True,
        append_images=quant64[1:],
        duration=durations,
        loop=0,
        optimize=True,
    )
    size_kb = os.path.getsize(out_path) / 1024
    print(f"Re-saved — {size_kb:.0f} KB")
