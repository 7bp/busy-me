"""Direct /dev/hidraw* access for USB busylights — no pip dependencies needed."""

from __future__ import annotations

import fcntl
import logging
import os
import struct

_LOGGER = logging.getLogger(__name__)

# Known devices: (vendor_id, product_id, name, protocol)
DEVICE_TABLE = [
    (0x16C0, 0x27DB, "MuteMe Original", "muteme"),
    (0x20A0, 0x42DA, "MuteMe Original", "muteme"),
    (0x04D8, 0xF848, "Kuando Busylight Alpha", "kuando"),
    (0x27BB, 0x3BCA, "Kuando Busylight Alpha", "kuando"),
    (0x27BB, 0x3BCD, "Kuando Busylight Omega", "kuando"),
    (0x04D8, 0xF372, "Luxafor Flag/Orb/Mute", "luxafor"),
    (0x27B8, 0x01ED, "ThingM Blink(1)", "blink1"),
]

HID_IOC_MAGIC = ord("H")
HIDIOCSFEATURE = 6  # IOC_OUT | _IOW(HID_IOC_MAGIC, 6, ...)


def _hidraw_paths() -> list[str]:
    """List /dev/hidraw* entries."""
    try:
        return ["/dev/" + e for e in os.listdir("/dev/") if e.startswith("hidraw")]
    except FileNotFoundError:
        return []


def _uevent(path: str) -> dict[str, str]:
    """Read uevent file for a hidraw device, return dict of k=v pairs."""
    # /sys/class/hidraw/hidrawN/device/uevent
    uevent_path = path.replace("/dev/", "/sys/class/hidraw/") + "/device/uevent"
    try:
        with open(uevent_path) as f:
            data = f.read()
        result = {}
        for line in data.strip().splitlines():
            if "=" in line:
                k, v = line.split("=", 1)
                result[k.strip()] = v.strip()
        return result
    except (FileNotFoundError, PermissionError, OSError):
        return {}


def _parse_hid_id(uevent: dict[str, str]) -> tuple[int, int] | None:
    """Extract (vendor_id, product_id) from uevent dict via HID_ID."""
    raw = uevent.get("HID_ID", "")
    # Format: bus:vendor:product  e.g. 0001:16C0:27DB
    parts = raw.split(":")
    if len(parts) == 3:
        try:
            return int(parts[1], 16), int(parts[2], 16)
        except ValueError:
            pass
    return None


def scan() -> list[dict]:
    """Scan /dev/hidraw* for known busylights.

    Returns list of dicts:
      {path, vid, pid, name, protocol, serial}
    """
    found = []
    dev_table = {(v, p): (n, pr) for v, p, n, pr in DEVICE_TABLE}

    for path in _hidraw_paths():
        ue = _uevent(path)
        ids = _parse_hid_id(ue)
        if ids is None:
            continue
        vid, pid = ids
        entry = dev_table.get((vid, pid))
        if entry is None:
            continue
        name, protocol = entry
        found.append({
            "path": path,
            "vid": vid,
            "pid": pid,
            "name": name,
            "protocol": protocol,
            "serial": ue.get("HID_UNIQ", ""),
        })

    return found


def send_muteme(dev_path: str, r: int, g: int, b: int) -> None:
    """Set MuteMe colour: 2-byte report [0x00, bitfield].

    Bit 0=R, 1=G, 2=B, 4=Dim, 5=Blink, 6=Sleep.
    Only the dominant channel lights up (no RGB mixing).
    """
    bits = _quantize(r, g, b)
    _hid_write(dev_path, bytes([0x00, bits]))


def send_muteme_with_effect(dev_path: str, r: int, g: int, b: int,
                             blink: bool = False, dim: bool = False,
                             sleep: bool = False) -> None:
    """Set MuteMe colour with hardware effect flags."""
    bits = _quantize(r, g, b)
    if blink:
        bits |= 0x20
    if dim:
        bits |= 0x10
    if sleep:
        bits |= 0x40
    _hid_write(dev_path, bytes([0x00, bits]))


def send_kuando(dev_path: str, r: int, g: int, b: int) -> None:
    """Set Kuando Busylight: 64-byte HID output report."""
    r_s = int(r * 100 / 255)
    g_s = int(g * 100 / 255)
    b_s = int(b * 100 / 255)
    buf = bytearray(64)
    buf[0] = 0x10  # opcode Jump in high nibble
    buf[2] = r_s
    buf[3] = g_s
    buf[4] = b_s
    csum = sum(buf[:62]) & 0xFFFF
    buf[62] = (csum >> 8) & 0xFF
    buf[63] = csum & 0xFF
    _hid_write(dev_path, bytes(buf))


def send_luxafor(dev_path: str, r: int, g: int, b: int) -> None:
    """Set Luxafor Flag/Orb: 5-byte report [cmd=0x01, leds=0xFF, R, G, B]."""
    _hid_write(dev_path, bytes([0x01, 0xFF, r, g, b]))


def send_blink1(dev_path: str, r: int, g: int, b: int, fade_ms: int = 100) -> None:
    """Set Blink(1): 8-byte feature report with Report ID 1."""
    report = bytes([
        0x01,  # report ID
        0x63,  # FadeColor action
        r, g, b,
        fade_ms & 0xFF,
        (fade_ms >> 8) & 0xFF,
        0x00,  # all LEDs
    ])
    _hid_feature(dev_path, report)


def send_off(dev_path: str, protocol: str) -> None:
    """Turn off the light — dispatches to the right protocol."""
    if protocol == "muteme":
        _hid_write(dev_path, bytes([0x00, 0x00]))
    elif protocol == "kuando":
        send_kuando(dev_path, 0, 0, 0)
    elif protocol == "luxafor":
        send_luxafor(dev_path, 0, 0, 0)
    elif protocol == "blink1":
        send_blink1(dev_path, 0, 0, 0)


# ── Low-level helpers ──

def _hid_write(path: str, data: bytes) -> None:
    """Write bytes to a HID device via /dev/hidraw*."""
    fd = os.open(path, os.O_RDWR)
    try:
        os.write(fd, data)
    finally:
        os.close(fd)


def _hid_feature(path: str, data: bytes) -> None:
    """Send a feature report via HIDIOCSFEATURE ioctl."""
    fd = os.open(path, os.O_RDWR)
    try:
        buf = bytearray(data)
        # HIDIOCSFEATURE(size) — the ioctl takes report number + data
        size = len(buf)
        ioc = _IOW(HID_IOC_MAGIC, HIDIOCSFEATURE, size)
        fcntl.ioctl(fd, ioc, buf, True)
    finally:
        os.close(fd)


def _IOW(group: int, num: int, size: int) -> int:
    """Build an _IOW ioctl request number."""
    return (2 << 30) | (size << 16) | (group << 8) | num


def _quantize(r: int, g: int, b: int) -> int:
    """Pick the dominant channel for 1-bit MuteMe colours."""
    if r == 0 and g == 0 and b == 0:
        return 0
    palette = [
        (0x01, 255, 0, 0),
        (0x02, 0, 255, 0),
        (0x04, 0, 0, 255),
        (0x03, 255, 255, 0),
        (0x05, 255, 0, 255),
        (0x06, 0, 255, 255),
        (0x07, 255, 255, 255),
    ]
    best, best_d = 0, float("inf")
    for bits, pr, pg, pb in palette:
        d = (r - pr) ** 2 + (g - pg) ** 2 + (b - pb) ** 2
        if d < best_d:
            best, best_d = bits, d
    return best
