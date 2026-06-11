"""Render VidFetch branding assets from the SVG sources in this folder.

Outputs:
  - src-tauri/icons/nsis-sidebar.bmp  (164x314, 24-bit, NSIS welcome/finish page)
  - src-tauri/icons/nsis-header.bmp   (150x57, 24-bit, NSIS header strip)
  - branding/banner.png               (2400x600, README hero banner)

The app icon set is generated separately: `npm run tauri icon -- branding/icon.svg`

Requires: pip install resvg-py pillow
"""

import io
from pathlib import Path

import resvg_py
from PIL import Image

ROOT = Path(__file__).resolve().parent.parent
BRANDING = ROOT / "branding"
ICONS = ROOT / "src-tauri" / "icons"


def render(svg: Path, width: int, height: int) -> Image.Image:
    png = bytes(
        resvg_py.svg_to_bytes(
            svg_path=str(svg), width=width, height=height, resources_dir=str(BRANDING)
        )
    )
    return Image.open(io.BytesIO(png))


def to_bmp(svg: Path, out: Path, width: int, height: int) -> None:
    # Render at 2x and downscale for cleaner anti-aliasing, then flatten:
    # NSIS bitmaps must be 24-bit with no alpha channel.
    img = render(svg, width * 2, height * 2).resize((width, height), Image.LANCZOS)
    background = Image.new("RGB", img.size, "#160c06")
    background.paste(img, mask=img.getchannel("A"))
    background.save(out, "BMP")
    print(f"wrote {out.relative_to(ROOT)}")


def main() -> None:
    to_bmp(BRANDING / "sidebar.svg", ICONS / "nsis-sidebar.bmp", 164, 314)
    to_bmp(BRANDING / "header.svg", ICONS / "nsis-header.bmp", 150, 57)

    banner = render(BRANDING / "banner.svg", 2400, 600)
    banner.save(BRANDING / "banner.png")
    print("wrote branding/banner.png")


if __name__ == "__main__":
    main()
