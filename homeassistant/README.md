# Busy Me — Home Assistant Integration

Home Assistant custom component for controlling USB busy lights (MuteMe, Kuando, Luxafor, ThingM Blink(1), etc.) as RGB light entities.

## Requirements (HA Docker / HA OS)

The `busylight-core` pip package needs the system `hidapi` library. In the HA container or host:

**Alpine-based images (HA Docker default, HA OS):**
```bash
docker exec homeassistant apk add libusb hidapi
```

**Debian-based hosts (Raspberry Pi OS + Docker):**
```bash
docker exec homeassistant apt update
docker exec homeassistant apt install -y libhidapi-dev
```

After installing, restart HA.

## Installation

1. **Copy the integration** into your HA `custom_components` directory:

   ```bash
   cp -r custom_components/busy_me_ha /path/to/config/custom_components/
   ```

2. **Add to `configuration.yaml`:**
   ```yaml
   light:
     - platform: busy_me_ha
   ```

3. **Plug in your USB busy light** and pass the device through to the container.

   Alternatively, add via the UI: **Settings → Devices & Services → Add Integration → search "Busy Me Light"**. This will scan your USB bus and create a config entry. After setup, click **Configure** on the entry to adjust poll interval and default effect.

   Find the HID device:
   ```bash
   ls /dev/hidraw*
   ```
   Unplug/replug the MuteMe and see which new device appears.

   Add to your Docker run or compose:
   ```yaml
   devices:
     - /dev/hidraw0:/dev/hidraw0
   ```

4. **Restart HA** and check the logs (`Settings → System → Logs`) for messages from `busy_me_ha`.

   If it works, you'll see: `Found N busy light(s): ...`
   If not, check the error message — it will tell you what's missing.

## Entities

One `light.busy_me_ha_*` entity is created per detected device. Supports:

- `turn_on` / `turn_off`
- `rgb_color` set on turn_on (mapped to nearest primary for MuteMe)
- `effect` — hardware effects (MuteMe only): `blink`, `dim`, `sleep`

## Services

| Service | Description |
|---------|-------------|
| `busy_me_ha.rescan` | Re-scan USB bus for connected lights |
| `light.turn_on` with `effect` | Set blink/dim/sleep on MuteMe devices |

## Example automations

### Red light when garage door is open
```yaml
automation:
  - trigger:
      - platform: state
        entity_id: cover.garage_door
        to: "open"
    action:
      - service: light.turn_on
        target:
          entity_id: light.busy_me_ha_muteme
        data:
          rgb_color: [255, 0, 0]
```

### Blink when motion detected at night
```yaml
automation:
  - trigger:
      - platform: state
        entity_id: binary_sensor.motion_sensor
        to: "on"
    condition:
      - condition: sun
        after: sunset
    action:
      - service: light.turn_on
        target:
          entity_id: light.busy_me_ha_muteme
        data:
          rgb_color: [255, 100, 0]
          effect: blink
```

## Notes

- **MuteMe**: 1-bit per color channel (on/off only). Colors are quantized to the nearest primary. Hardware effects (blink, dim, sleep) are supported.
- **Other devices**: Full 8-bit RGB. Effects are not currently implemented.
- The integration requires `busylight-core>=2.4.0` from PyPI.
