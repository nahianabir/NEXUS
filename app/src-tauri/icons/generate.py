"""
Generate NEXUS app icons (PNG + ICO) without external image libraries.

Outputs:
  - icon.png   (256x256)
  - 32x32.png
  - 128x128.png
  - 128x128@2x.png (256x256)
  - icon.ico   (32, 64, 128, 256 frames)

Pure stdlib, so the build does not need Pillow.
"""
import os, struct, zlib, io

OUT = os.path.dirname(os.path.abspath(__file__))


def _png_chunk(tag, data):
    return (
        struct.pack(">I", len(data))
        + tag
        + data
        + struct.pack(">I", zlib.crc32(tag + data) & 0xFFFFFFFF)
    )


def make_png(size, pixels):
    """pixels: list of (R, G, B, A) tuples, size*size length, row-major."""
    raw = bytearray()
    for y in range(size):
        raw.append(0)  # filter: None
        row = bytearray()
        for x in range(size):
            r, g, b, a = pixels[y * size + x]
            row += bytes((r, g, b, a))
        raw += row
    compressed = zlib.compress(bytes(raw), 9)
    sig = b"\x89PNG\r\n\x1a\n"
    ihdr = struct.pack(">IIBBBBB", size, size, 8, 6, 0, 0, 0)
    return sig + _png_chunk(b"IHDR", ihdr) + _png_chunk(b"IDAT", compressed) + _png_chunk(b"IEND", b"")


def draw_icon(size):
    """Draws a flat dark background with a cyan NEXUS ring and accent."""
    pixels = [None] * (size * size)
    cx = cy = (size - 1) / 2.0
    outer = size * 0.46
    inner = size * 0.32
    dot = size * 0.10

    # Background: dark gradient charcoal to near-black.
    for y in range(size):
        for x in range(size):
            t = (x + y) / (2 * size)
            r = int(18 + 6 * t)
            g = int(22 + 8 * t)
            b = int(28 + 10 * t)
            pixels[y * size + x] = (r, g, b, 255)

    # Cyan ring + amber accent + central dot.
    for y in range(size):
        for x in range(size):
            d = ((x - cx) ** 2 + (y - cy) ** 2) ** 0.5
            r, g, b, a = pixels[y * size + x]
            if abs(d - outer) < 1.2:
                pixels[y * size + x] = (96, 220, 255, 255)
            elif abs(d - inner) < 0.9:
                pixels[y * size + x] = (60, 170, 220, 255)
            elif d < dot:
                pixels[y * size + x] = (255, 184, 92, 255)
            elif abs(d - (outer * 0.78)) < 0.6 and d < outer * 0.82:
                pixels[y * size + x] = (200, 160, 255, 255)

    # Soft diagonal sheen across the upper-left quadrant.
    for y in range(size):
        for x in range(size):
            if x + y < size * 0.55:
                r, g, b, a = pixels[y * size + x]
                pixels[y * size + x] = (min(255, r + 8), min(255, g + 10), min(255, b + 14), a)

    return pixels


def make_ico(sizes, pixels_per_size):
    """Embed multiple PNGs into an ICO container (Vista+)."""
    out = io.BytesIO()
    out.write(struct.pack("<HHH", 0, 1, len(sizes)))
    offset = 6 + 16 * len(sizes)
    entries = bytearray()
    image_data = bytearray()
    for size, png_bytes in zip(sizes, pixels_per_size):
        w = 0 if size >= 256 else size
        h = 0 if size >= 256 else size
        entries += struct.pack(
            "<BBBBHHII",
            w, h, 0, 0, 1, 32, len(png_bytes), offset,
        )
        image_data += png_bytes
        offset += len(png_bytes)
    out.write(entries)
    out.write(image_data)
    return out.getvalue()


def main():
    sizes = [32, 64, 128, 256]
    png_bytes_per_size = []
    for s in sizes:
        pixels = draw_icon(s)
        png_bytes_per_size.append(make_png(s, pixels))
        if s == 32:
            with open(os.path.join(OUT, "32x32.png"), "wb") as f:
                f.write(png_bytes_per_size[-1])
        if s == 128:
            with open(os.path.join(OUT, "128x128.png"), "wb") as f:
                f.write(png_bytes_per_size[-1])
        if s == 256:
            with open(os.path.join(OUT, "icon.png"), "wb") as f:
                f.write(png_bytes_per_size[-1])
            with open(os.path.join(OUT, "128x128@2x.png"), "wb") as f:
                f.write(png_bytes_per_size[-1])

    ico = make_ico(sizes, png_bytes_per_size)
    with open(os.path.join(OUT, "icon.ico"), "wb") as f:
        f.write(ico)

    print("icons written:", sorted(os.listdir(OUT)))


if __name__ == "__main__":
    main()
