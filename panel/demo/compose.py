"""Puts the panel (left) and the VectorCraft window (right) side by side under a caption: compose.py panel.png vc.png out.png "Title" "Subtitle" """
import sys
from PIL import Image, ImageDraw, ImageFont

panel_png, vc_png, out_png, title, subtitle = sys.argv[1:6]
panel = Image.open(panel_png).convert("RGB")
vc = Image.open(vc_png).convert("RGB")
H = 1800
panel = panel.resize((round(panel.width * H / panel.height), H))
vc = vc.resize((round(vc.width * H / vc.height), H))
bar = 190
W = panel.width + vc.width
img = Image.new("RGB", (W, H + bar), (24, 26, 29))
img.paste(panel, (0, bar))
img.paste(vc, (panel.width, bar))
d = ImageDraw.Draw(img)
def font(size):
    for f in ("/System/Library/Fonts/Helvetica.ttc", "/System/Library/Fonts/SFNS.ttf", "/Library/Fonts/Arial.ttf"):
        try:
            return ImageFont.truetype(f, size)
        except OSError:
            pass
    return ImageFont.load_default()
d.text((48, 28), title, fill=(255, 255, 255), font=font(64))
d.text((48, 112), subtitle, fill=(170, 176, 184), font=font(40))
d.text((48, bar - 6), "", font=font(10))
d.line([(panel.width, bar), (panel.width, H + bar)], fill=(90, 94, 100), width=4)
d.text((20, H + bar - 52), "Road Builder panel", fill=(40, 40, 40), font=font(34))
d.text((panel.width + 20, H + bar - 52), "VectorCraft", fill=(40, 40, 40), font=font(34))
img.save(out_png)
