import os
from PIL import Image, ImageDraw

def create_icon(size):
    img = Image.new("RGBA", (size, size), (0, 0, 0, 0))
    draw = ImageDraw.Draw(img)

    pad = size * 0.06
    # Outer dark rounded box with neon lime border
    draw.rounded_rectangle(
        [pad, pad, size - pad, size - pad],
        radius=size * 0.22,
        fill=(10, 14, 11, 255),
        outline=(0, 255, 102, 255),
        width=max(1, int(size * 0.05))
    )

    # Glowing pulse / lightning download symbol
    # Center coords
    cx, cy = size / 2.0, size / 2.0
    scale = size / 100.0

    # Lightning bolt pointing downward
    points = [
        (cx + 2 * scale, cy - 30 * scale),
        (cx - 18 * scale, cy - 2 * scale),
        (cx - 3 * scale, cy - 2 * scale),
        (cx - 12 * scale, cy + 28 * scale),
        (cx + 20 * scale, cy - 2 * scale),
        (cx + 5 * scale, cy - 2 * scale),
    ]
    draw.polygon(points, fill=(0, 255, 102, 255))

    # Inner bright core
    inner_points = [
        (cx + 2 * scale, cy - 24 * scale),
        (cx - 13 * scale, cy - 2 * scale),
        (cx - 2 * scale, cy - 2 * scale),
        (cx - 8 * scale, cy + 20 * scale),
        (cx + 15 * scale, cy - 2 * scale),
        (cx + 4 * scale, cy - 2 * scale),
    ]
    draw.polygon(inner_points, fill=(240, 255, 245, 220))

    return img

os.makedirs(r"d:\faster download\extension\icons", exist_ok=True)
os.makedirs(r"d:\faster download\pulsedl\src-tauri\icons", exist_ok=True)

# Extension icons
for s in [16, 48, 128]:
    img = create_icon(s)
    img.save(rf"d:\faster download\extension\icons\icon{s}.png")

# Tauri icons
img32 = create_icon(32)
img32.save(r"d:\faster download\pulsedl\src-tauri\icons\32x32.png")

img128 = create_icon(128)
img128.save(r"d:\faster download\pulsedl\src-tauri\icons\128x128.png")

img256 = create_icon(256)
img256.save(r"d:\faster download\pulsedl\src-tauri\icons\128x128@2x.png")
img256.save(r"d:\faster download\pulsedl\src-tauri\icons\icon.png")

# Windows ICO file containing multiple resolutions
img256.save(
    r"d:\faster download\pulsedl\src-tauri\icons\icon.ico",
    format="ICO",
    sizes=[(16, 16), (24, 24), (32, 32), (48, 48), (64, 64), (128, 128), (256, 256)]
)

print("Icons generated successfully!")
