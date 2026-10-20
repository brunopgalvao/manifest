"""Tier 3 reference: a figure drawn by an independent rasterizer (PIL, 4x supersampled), to be reproduced
with MX v2 paint primitives and compared within tolerance. Writes ref/vector.png (400x300)."""
import pathlib
from PIL import Image, ImageDraw
S = 4
W, H = 400, 300
im = Image.new("RGB", (W * S, H * S), "#ffffff")
d = ImageDraw.Draw(im)
sc = lambda *v: [x * S for x in v]
d.rounded_rectangle(sc(20, 20, 220, 120), radius=12 * S, fill="#2f5bea")
d.rectangle(sc(240, 20, 380, 60), fill="#e1f4e8", outline="#156e3b", width=2 * S)
d.ellipse(sc(300 - 40, 180 - 40, 300 + 40, 180 + 40), fill="#c62a2a")
d.ellipse(sc(300 - 20, 180 - 20, 300 + 20, 180 + 20), fill="#ffffff")
d.line(sc(20, 280, 380, 140), fill="#1b1d21", width=3 * S)
d.polygon(sc(60, 160, 200, 160, 130, 260), fill="#8a5900")
d.rectangle(sc(40, 200, 48, 208), fill="#0b7285")
im = im.resize((W, H), Image.LANCZOS)
out = pathlib.Path(__file__).parent / "ref" / "vector.png"
im.save(out)
print("wrote", out)
