# Busy Me

**A lightweight system tray app that monitors your microphone, speakers, and webcam — and controls a USB busy light to match.**

Built with Rust. Runs on macOS (ARM + x64) and Windows.

The tray icon shows a colored **ring** that changes based on your device activity. No cloud, no recording, no data leaves your machine.

## How it works

1. The app sits in your menu bar as a colored ring
2. Every second it checks if any app is using your mic, speakers, or webcam
3. **Mic or webcam active** (call, recording) → ring turns **red**
4. **Only speakers active** (music, video) → ring turns **orange**
5. **Nothing active** → ring stays **green**

## Based on busylight-for-humans

The USB busylight protocols are derived from the excellent [busylight-for-humans](https://github.com/JnyJny/busylight) Python library by JnyJny — a unified interface for 26+ USB status lights from 9 vendors. This Rust port adapts the HID protocols for three popular device families while keeping the same core semantics: `on(color)`, `off()`, and keepalive management.

The audio monitoring (the "why" behind changing the light) is original to this project.

## Do I need a busylight?

**No.** The tray icon works standalone — it shows your call status without any extra hardware. The busylight is an optional output device.

The "No busylight device found" warning at startup is normal if you don't have one plugged in. The tray icon and audio monitoring still work.

To use a busylight:

1. Plug in a compatible USB light
2. Run the app — it auto-detects supported devices
3. The light syncs with your tray icon

| Vendor | Devices |
|--------|---------|
| **Kuando** | Busylight Alpha, Busylight Omega |
| **EPOS** | Busylight |
| **Luxafor** | Flag, Orb, Mute |
| **MuteMe** | MuteMe Original (tested), MuteMe Mini |
| **ThingM** | Blink(1), Blink(1) mk2 |

If your device isn't listed, the tray icon still works — you just won't get the USB light control.

## Quick Start

```bash
# Run directly (works without a busylight)
cargo run

# Build release bundle for macOS
make bundle-darwin
open target/release/bundle-darwin/Busy\ Me.app
```

## Monitoring

**Microphone & Speaker (macOS):** Uses `CoreAudio` to query the default input/output devices via `kAudioDevicePropertyDeviceIsRunningSomewhere`. When any app opens an audio stream, the property returns true — no audio capture needed, no permissions required.

**Webcam (macOS):** Checks `AppleCameraInterface` / `AppleH13CamIn` IOKit services via `ioreg` for active client connections. When any app has the camera open, an `IOUserClient` entry appears. No camera access required.

**Windows:** Uses WASAPI `IAudioSessionManager2` to enumerate active audio sessions on the default capture and render endpoints. When any app has an active audio session, the input (mic) or output (speaker) is considered busy.

## Busylight Protocols

All derived from the [busylight-core](https://github.com/JnyJny/busylight/tree/main/packages/busylight-core) Python library:

- **Kuando Busylight** — 64-byte HID output report containing 7 step words (8 bytes each) + a footer with checksum. Requires periodic keepalive every 10s or the device turns off. RGB values are scaled from 0–255 to the device's internal 0–100 range.
- **Luxafor Flag/Orb/Mute** — 5-byte HID output report: `[command(0x01), leds(0xFF=all), R, G, B]`. Also supports fade, strobe, wave, and built-in pattern commands.
- **ThingM Blink(1)** — 8-byte HID feature report with Report ID 1. Uses ASCII action bytes (`'c'` = FadeColor, `'n'` = SetColor). Supports fade timing and 16-line pattern memory.
- **MuteMe Original** — 2-byte HID output report `[0x00, bitfield]` where bit 0=Red, 1=Green, 2=Blue. Simple on/off color control (1 bit per channel). Also supports dim, blink, and sleep via bits 4–6.

## Configuration

All settings are available from the tray menu (right-click the icon):

- **Enable Monitoring** — toggle monitoring on/off
- **Colors** → choose colors for On Call (mic/cam), Free, and Speaker states
- **Poll Interval** — how often to check audio state (0.5s–3s)

The config is also stored as JSON at `$HOME/.config/busy-me/config.json`:

```json
{
  "enabled": true,
  "poll_interval_ms": 1000,
  "busy_color": [255, 40, 40],
  "free_color": [40, 230, 40],
  "speaker_color": [255, 160, 40]
}
```

## Building

```bash
# Development
cargo run

# Release
cargo build --release

# macOS .app bundle + DMG (drag-to-Applications installer)
make dmg
# Opens target/aarch64-apple-darwin/release/Busy Me.dmg

# Cross-compile (requires appropriate toolchains)
make build-darwin-arm64    # Apple Silicon
make build-darwin-x64      # Intel Mac
make build-windows-x64     # Windows
```

## Webhook → Home Assistant

Toggle **Webhook → HA** in the tray menu to send state changes as HTTP POSTs to a Home Assistant webhook. Configure the URL in `config.json`:

```json
{
  "webhook_enabled": true,
  "webhook_url_free": "http://homeassistant.local:8123/api/webhook/busy_me_free",
  "webhook_url_speaker": "http://homeassistant.local:8123/api/webhook/busy_me_speaker",
  "webhook_url_busy": "http://homeassistant.local:8123/api/webhook/busy_me_busy"
}
```

Each state fires its own webhook URL so you can create three separate HA automations, one per state. The POST is empty — the URL itself identifies the state.

**Setup in HA:**
1. Go to **Settings → Automations → Create Automation → Webhook**
2. Create three automations, one for each webhook ID (`busy_me_free`, `busy_me_speaker`, `busy_me_busy`)
3. Add actions to each — flash lights red when busy, play a chime when free, etc.

The webhook thread debounces rapid flapping (2-second stability window) so you only get clean state transitions.

## License

Apache-2.0
