"""Generate placeholder icon files for Tauri.

Creates src-tauri/icons/* as a flat gradient with "VF" in the center.
Replace these with a real icon before shipping (Phase 8).
"""

from __future__ import annotations

from pathlib import Path

from PIL import Image, ImageDraw, ImageFont

OUT = Path(__file__).resolve().parent.parent / "src-tauri" / "icons"
OUT.mkdir(parents=True, exist_ok=True)


def make_base(size: int) -> Image.Image:
    img = Image.new("RGBA", (size, size), (0, 0, 0, 0))
    draw = ImageDraw.Draw(img)

    # Rounded square background with a warm accent (fox nod :3)
    radius = size // 6
    draw.rounded_rectangle(
        [(0, 0), (size - 1, size - 1)],
        radius=radius,
        fill=(255, 140, 66, 255),
    )

    # Text: "VF"
    try:
        font = ImageFont.truetype("arial.ttf", size=int(size * 0.45))
    except Exception:
        font = ImageFont.load_default()
    text = "VF"
    bbox = draw.textbbox((0, 0), text, font=font)
    tw = bbox[2] - bbox[0]
    th = bbox[3] - bbox[1]
    draw.text(
        ((size - tw) / 2 - bbox[0], (size - th) / 2 - bbox[1] - size * 0.02),
        text,
        font=font,
        fill=(26, 15, 8, 255),
    )
    return img


def main() -> None:
    sizes = {
        "32x32.png": 32,
        "128x128.png": 128,
        "128x128@2x.png": 256,
        "icon.png": 512,
    }
    for name, size in sizes.items():
        make_base(size).save(OUT / name)
        print(f"wrote {name} ({size}x{size})")

    # Windows .ico (multi-res)
    ico_sizes = [(16, 16), (32, 32), (48, 48), (64, 64), (128, 128), (256, 256)]
    base = make_base(256)
    base.save(OUT / "icon.ico", format="ICO", sizes=ico_sizes)
    print("wrote icon.ico")

    # macOS .icns — Pillow supports ICNS write on all platforms
    try:
        make_base(512).save(OUT / "icon.icns", format="ICNS")
        print("wrote icon.icns")
    except Exception as e:
        print(f"skipped icon.icns: {e}")


if __name__ == "__main__":
    main()
