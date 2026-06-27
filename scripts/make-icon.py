"""Generate a simple red circle PNG for the app icon."""
import struct, zlib, sys

def create_circle_png(path, size, color):
    cx = cy = size / 2.0
    r = cx - 12
    raw = b''
    for y in range(size):
        raw += b'\x00'
        for x in range(size):
            dx, dy = x - cx, y - cy
            dist = (dx*dx + dy*dy) ** 0.5
            if dist <= r:
                t = min(1.0, r - dist + 1.5)
                a = int(min(255, 255 * max(0, min(1, t))))
                raw += struct.pack('BBB', *color) + struct.pack('B', a)
            else:
                raw += b'\x00\x00\x00\x00'

    def chunk(t, d):
        c = t + d
        return struct.pack('>I', len(d)) + c + struct.pack('>I', zlib.crc32(c) & 0xffffffff)

    with open(path, 'wb') as f:
        f.write(b'\x89PNG\r\n\x1a\n')
        f.write(chunk(b'IHDR', struct.pack('>IIBBBBB', size, size, 8, 6, 0, 0, 0)))
        f.write(chunk(b'IDAT', zlib.compress(raw)))
        f.write(chunk(b'IEND', b''))
    print(f'Created {path} ({size}x{size})')

if __name__ == '__main__':
    create_circle_png(sys.argv[1], int(sys.argv[2]), (200, 60, 60))
