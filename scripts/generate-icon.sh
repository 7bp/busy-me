#!/bin/bash
# Generate a macOS .icns app icon for Busy Me
# Requires: imagemagick or macOS built-in tools

set -e
OUT="Resources"
mkdir -p "$OUT"

# Generate a 1024x1024 PNG using basic math + sips
# We create a simple colored circle with padding

# Create a 1024x1024 red circle as base icon
python3 -c "
import struct, zlib

def create_png(width, height, color):
    def write_chunk(chunk_type, data):
        chunk = chunk_type + data
        return struct.pack('>I', len(data)) + chunk + struct.pack('>I', zlib.crc32(chunk) & 0xffffffff)

    # PNG signature
    sig = b'\x89PNG\r\n\x1a\n'

    # IHDR
    ihdr_data = struct.pack('>IIBBBBB', width, height, 8, 2, 0, 0, 0)
    ihdr = write_chunk(b'IHDR', ihdr_data)

    # IDAT (raw image data)
    raw = b''
    cy, cx = height / 2, width / 2
    r = min(cy, cx) - 20  # radius with padding
    for y in range(height):
        raw += b'\x00'  # filter byte
        for x in range(width):
            dx, dy = x - cx, y - cy
            dist = (dx*dx + dy*dy) ** 0.5
            if dist <= r:
                edge = max(0, min(1, r - dist + 1))
                alpha = int(min(255, 255 * edge))
                raw += struct.pack('BBB', *color) + struct.pack('B', alpha)
            else:
                raw += b'\x00\x00\x00\x00'

    idat = write_chunk(b'IDAT', zlib.compress(raw))
    iend = write_chunk(b'IEND', b'')
    return sig + ihdr + idat + iend

# Generate icon PNGs
colors = {
    'icon': (220, 50, 50),     # Reddish
}

png_data = create_png(1024, 1024, colors['icon'])
with open(f'$OUT/icon.png', 'wb') as f:
    f.write(png_data)

print('Generated $OUT/icon.png')
"

# Create iconset directory
ICONSET="$OUT/AppIcon.iconset"
mkdir -p "$ICONSET"

# Generate all required sizes
for SIZE in 16 32 64 128 256 512; do
    sips -z $SIZE $SIZE "$OUT/icon.png" --out "$ICONSET/icon_${SIZE}x${SIZE}.png" > /dev/null 2>&1
    sips -z $((SIZE*2)) $((SIZE*2)) "$OUT/icon.png" --out "$ICONSET/icon_${SIZE}x${SIZE}@2x.png" > /dev/null 2>&1
done

# Create icns
iconutil -c icns "$ICONSET" -o "$OUT/icon.icns"

echo "Generated $OUT/icon.icns"
rm -rf "$ICONSET" "$OUT/icon.png"
echo "Done"
