use hidapi::{HidApi, HidDevice};
use log::{info, warn};
use std::time::{Duration, Instant};

/// Known busylight vendor/product IDs from the busylight library
const DEVICES: &[(u16, u16, &str, DeviceKind)] = &[
    // Kuando Busylight Alpha
    (0x04D8, 0xF848, "Kuando Busylight Alpha (1)", DeviceKind::Kuando),
    (0x27BB, 0x3BCA, "Kuando Busylight Alpha (2)", DeviceKind::Kuando),
    (0x27BB, 0x3BCB, "Kuando Busylight Alpha (3)", DeviceKind::Kuando),
    (0x27BB, 0x3BCE, "Kuando Busylight Alpha (4)", DeviceKind::Kuando),
    // Kuando Busylight Omega
    (0x27BB, 0x3BCD, "Kuando Busylight Omega (1)", DeviceKind::Kuando),
    (0x27BB, 0x3BCF, "Kuando Busylight Omega (2)", DeviceKind::Kuando),
    // EPOS Busylight (same protocol as Kuando)
    (0x27BB, 0x3BC8, "EPOS Busylight", DeviceKind::Kuando),
    // Luxafor Flag / Orb / Mute (all share VID:PID, differ by product string)
    (0x04D8, 0xF372, "Luxafor Flag/Orb/Mute", DeviceKind::Luxafor),
    // ThingM Blink(1)
    (0x27B8, 0x01ED, "ThingM Blink(1)", DeviceKind::Blink1),
    // MuteMe Original
    (0x16C0, 0x27DB, "MuteMe Original (1)", DeviceKind::MuteMe),
    (0x20A0, 0x42DA, "MuteMe Original (2)", DeviceKind::MuteMe),
];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum DeviceKind {
    Kuando,
    Luxafor,
    Blink1,
    MuteMe,
}

pub struct Controller {
    api: HidApi,
    device: Option<HidDevice>,
    kind: Option<DeviceKind>,
    last_keepalive: Option<Instant>,
    color: [u8; 3],
    /// Effect flags for MuteMe: bit 4=Dim, bit 5=Blink, bit 6=Sleep
    muteme_effect: u8,
}

#[allow(dead_code)]
#[derive(Debug, Clone, Copy)]
enum LuxaforCmd {
    Color = 0x01,
    Fade = 0x02,
}

impl Controller {
    pub fn new() -> Self {
        let api = HidApi::new().expect("failed to initialize HID API");
        let mut ctrl = Self {
            api,
            device: None,
            kind: None,
            last_keepalive: None,
            color: [0, 0, 0],
            muteme_effect: 0,
        };
        ctrl.scan();
        ctrl
    }

    pub fn scan(&mut self) {
        for &(vid, pid, name, kind) in DEVICES {
            if let Ok(device) = self.api.open(vid, pid) {
                device.set_blocking_mode(true).ok();
                info!("Found device: {} ({:04X}:{:04X})", name, vid, pid);
        self.device = Some(device);
        self.kind = Some(kind);
        self.color = [0, 0, 0];
        info!("Busylight ready — running startup blink");
        self.startup_blink();
        return;
            }
        }
        warn!("No busylight device found");
        self.device = None;
        self.kind = None;
    }

    pub fn is_connected(&self) -> bool {
        self.device.is_some()
    }

    pub fn set_color(&mut self, r: u8, g: u8, b: u8) {
        self.color = [r, g, b];
        let _ = self.send_color(r, g, b);
    }

    /// Smoothly fade from the current color to a target color.
    /// For full-RGB devices (Kuando, Luxafor, Blink1) it steps through
    /// interpolated values so the eye sees a dim→crossfade→bright transition.
    /// For MuteMe (1-bit channels only) it does a brief off→on blink.
    pub fn fade_to_color(&mut self, r: u8, g: u8, b: u8) {
        if !self.is_connected() || self.color == [r, g, b] {
            self.color = [r, g, b];
            return;
        }
        let prev = self.color;

        if self.kind == Some(DeviceKind::MuteMe) {
            // MuteMe has full and dim (bit 4). We stretch the arc:
            //   full old → dim old → off → dim new → full new
            let dim = 0x10;
            let off_bits = 0x00;

            // 1. full old (pause to establish current color)
            let _ = self.device.as_ref().and_then(|d| d.write(&[0x00, Self::quantize_muteme(prev[0], prev[1], prev[2]) | self.muteme_effect]).ok());
            std::thread::sleep(std::time::Duration::from_millis(40));

            // 2. dim old (slow fade down)
            let _ = self.device.as_ref().and_then(|d| d.write(&[0x00, Self::quantize_muteme(prev[0], prev[1], prev[2]) | dim | self.muteme_effect]).ok());
            std::thread::sleep(std::time::Duration::from_millis(160));

            // 3. off (black gap)
            let _ = self.device.as_ref().and_then(|d| d.write(&[0x00, off_bits]).ok());
            std::thread::sleep(std::time::Duration::from_millis(80));

            // 4. dim new (slow fade up)
            self.color = [r, g, b];
            let _ = self.device.as_ref().and_then(|d| d.write(&[0x00, Self::quantize_muteme(r, g, b) | dim | self.muteme_effect]).ok());
            std::thread::sleep(std::time::Duration::from_millis(160));

            // 5. full new
            let _ = self.device.as_ref().and_then(|d| d.write(&[0x00, Self::quantize_muteme(r, g, b) | self.muteme_effect]).ok());
            return;
        }

        let steps = 6;

        for i in 0..=steps {
            let t = i as f32 / steps as f32;
            let sr = (prev[0] as f32 + (r as f32 - prev[0] as f32) * t) as u8;
            let sg = (prev[1] as f32 + (g as f32 - prev[1] as f32) * t) as u8;
            let sb = (prev[2] as f32 + (b as f32 - prev[2] as f32) * t) as u8;
            if self.send_color(sr, sg, sb).is_err() {
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(25));
        }
        self.color = [r, g, b];
    }

    pub fn off(&mut self) {
        self.color = [0, 0, 0];
        let _ = self.send_color(0, 0, 0);
    }

    pub fn send_keepalive(&mut self) {
        if self.kind == Some(DeviceKind::Kuando) {
            let _ = self.send_kuando_keepalive();
        }
    }

    /// Called periodically to keep the device alive (Kuando needs this)
    pub fn tick(&mut self) {
        let now = Instant::now();
        let needs_keepalive = self.last_keepalive
            .map(|t| now.duration_since(t) > Duration::from_secs(10))
            .unwrap_or(true);

        if needs_keepalive && self.color != [0, 0, 0] && self.kind == Some(DeviceKind::Kuando) {
            self.last_keepalive = Some(now);
            self.send_keepalive();
        }
    }

    fn send_color(&self, r: u8, g: u8, b: u8) -> Result<(), ()> {
        let device = self.device.as_ref().ok_or(())?;
        match self.kind {
            Some(DeviceKind::Kuando) => self.send_kuando_color(device, r, g, b),
            Some(DeviceKind::Luxafor) => self.send_luxafor_color(device, r, g, b),
            Some(DeviceKind::Blink1) => self.send_blink1_color(device, r, g, b),
            Some(DeviceKind::MuteMe) => self.send_muteme_color(device, r, g, b),
            None => Err(()),
        }
    }

    // ── Kuando Busylight ──

    /// Build a 64-byte HID report for Kuando Busylight.
    /// Format (from busylight library analysis):
    ///   7 Step objects (8 bytes each) + 1 Footer (8 bytes) = 64 bytes
    ///   Step: [opcode(4) | operand(4) | repeat(8) | R(8) | G(8) | B(8) | duty_on(8) | duty_off(8) | update(1) | ringtone(4) | volume(3)]
    fn build_kuando_report(r: u8, g: u8, b: u8, opcode: u8) -> [u8; 64] {
        let mut buf = [0u8; 64];
        let r_scaled = (r as u16 * 100 / 255) as u8;
        let g_scaled = (g as u16 * 100 / 255) as u8;
        let b_scaled = (b as u16 * 100 / 255) as u8;

        // Step 0: Jump/Set color
        // Big-endian 64-bit word byte layout:
        //   byte 0: [opcode(4) | operand(4)]
        //   byte 1: [repeat(8)]
        //   byte 2: [R(8)]
        //   byte 3: [G(8)]
        //   byte 4: [B(8)]
        //   byte 5: [duty_cycle_on(8)]
        //   byte 6: [duty_cycle_off(8)]
        //   byte 7: [0|update(1)|ringtone(4)|volume(3)]
        buf[0] = opcode << 4;           // opcode | operand (operand=0)
        buf[1] = 0;                     // repeat
        buf[2] = r_scaled;              // R
        buf[3] = g_scaled;              // G
        buf[4] = b_scaled;              // B
        buf[5] = 0;                     // duty_cycle_on
        buf[6] = 0;                     // duty_cycle_off
        buf[7] = 0;                     // update/ringtone/volume

        // Steps 1-6: unused (zeros)

        // Footer (bytes 56-63)
        // checksum = sum of all bytes in steps + footer (excluding checksum itself)
        let sum: u16 = buf[..62].iter().map(|&x| x as u16).sum();
        buf[62] = (sum >> 8) as u8;
        buf[63] = (sum & 0xFF) as u8;

        buf
    }

    fn send_kuando_color(&self, device: &HidDevice, r: u8, g: u8, b: u8) -> Result<(), ()> {
        let report = Self::build_kuando_report(r, g, b, 0x1); // OpCode::Jump
        device.write(&report).map_err(|_| ())?;
        Ok(())
    }

    fn send_kuando_keepalive(&self) -> Result<(), ()> {
        let device = self.device.as_ref().ok_or(())?;
        let report = Self::build_kuando_report(0, 0, 0, 0x8); // OpCode::KeepAlive
        device.write(&report).map_err(|_| ())?;
        Ok(())
    }

    // ── Luxafor Flag/Orb/Mute ──

    /// Build a 5+ byte HID report for Luxafor devices.
    /// Format: [command(1), leds(1), R(1), G(1), B(1)]
    ///   command = 0x01 for solid color
    ///   leds = 0xFF for all LEDs
    fn send_luxafor_color(&self, device: &HidDevice, r: u8, g: u8, b: u8) -> Result<(), ()> {
        let report = [0x01, 0xFF, r, g, b];
        device.write(&report).map_err(|_| ())?;
        Ok(())
    }

    // ── ThingM Blink(1) ──

    /// Build an 8-byte feature report for Blink(1).
    /// Format: [report_id(1)=0x01, action(1)='c'=0x63, R(1), G(1), B(1), fade_lo(1), fade_hi(1), leds(1)=0x00]
    fn build_blink1_report(r: u8, g: u8, b: u8) -> [u8; 8] {
        let fade_ms: u16 = 100;
        [
            0x01,
            0x63, // action: FadeColor ('c')
            r, g, b,
            (fade_ms & 0xFF) as u8,
            (fade_ms >> 8) as u8,
            0x00, // leds: All
        ]
    }

    fn send_blink1_color(&self, device: &HidDevice, r: u8, g: u8, b: u8) -> Result<(), ()> {
        let report = Self::build_blink1_report(r, g, b);
        // Blink(1) uses feature reports (send_feature_report), not write
        device.send_feature_report(&report).map_err(|_| ())?;
        Ok(())
    }

    /// Show a quick RGB blink to confirm the device is alive.
    fn startup_blink(&mut self) {
        let colors: [[u8; 3]; 3] = [[255, 0, 0], [0, 255, 0], [0, 0, 255]];
        if let Some(ref device) = self.device {
            for &c in &colors {
                let _ = self.send_color_inner(device, c[0], c[1], c[2]);
                std::thread::sleep(std::time::Duration::from_millis(120));
            }
            let _ = device.write(&[0x00, 0x00]);
        }
        self.color = [0, 0, 0];
    }

    /// Send color to a specific device handle without routing through self.device.
    /// Used by startup_blink and scan to avoid borrow conflicts.
    fn send_color_inner(&self, device: &HidDevice, r: u8, g: u8, b: u8) -> Result<(), ()> {
        match self.kind {
            Some(DeviceKind::Kuando) => self.send_kuando_color(device, r, g, b),
            Some(DeviceKind::Luxafor) => self.send_luxafor_color(device, r, g, b),
            Some(DeviceKind::Blink1) => self.send_blink1_color(device, r, g, b),
            Some(DeviceKind::MuteMe) => self.send_muteme_color(device, r, g, b),
            None => Err(()),
        }
    }

    // ── MuteMe Original ──

    /// Map an RGB color to the closest of the 8 MuteMe bitfield values.
    /// MuteMe LEDs are on/off per channel — no intensity. We find the
    /// nearest primary/secondary color in RGB space so that "orange"
    /// becomes yellow (R+G) rather than falling back to red alone.
    fn quantize_muteme(r: u8, g: u8, b: u8) -> u8 {
        if r == 0 && g == 0 && b == 0 {
            return 0;
        }
        // 8 possible colors the MuteMe can display
        const PALETTE: [(u8, [u8; 3]); 7] = [
            (0x01, [255, 0, 0]),   // red
            (0x02, [0, 255, 0]),   // green
            (0x04, [0, 0, 255]),   // blue
            (0x03, [255, 255, 0]), // yellow
            (0x05, [255, 0, 255]), // magenta
            (0x06, [0, 255, 255]), // cyan
            (0x07, [255, 255, 255]), // white
        ];
        let rf = r as f32;
        let gf = g as f32;
        let bf = b as f32;
        PALETTE
            .iter()
            .map(|&(bits, c)| {
                let d = (rf - c[0] as f32).powi(2)
                    + (gf - c[1] as f32).powi(2)
                    + (bf - c[2] as f32).powi(2);
                (bits, d)
            })
            .min_by(|a, b| a.1.partial_cmp(&b.1).unwrap())
            .map(|(bits, _)| bits)
            .unwrap_or(0)
    }

    fn build_muteme_report(r: u8, g: u8, b: u8, effect: u8) -> [u8; 2] {
        let color = Self::quantize_muteme(r, g, b);
        [0x00, color | effect]
    }

    fn send_muteme_color(&self, device: &HidDevice, r: u8, g: u8, b: u8) -> Result<(), ()> {
        let report = Self::build_muteme_report(r, g, b, self.muteme_effect);
        device.write(&report).map_err(|_| ())?;
        Ok(())
    }

    /// Enable or disable hardware blink on the MuteMe device.
    /// When blink is on, the light automatically flashes at the device's
    /// internal rate — no polling needed from the host.
    pub fn set_muteme_blink(&mut self, enabled: bool) {
        if enabled {
            self.muteme_effect |= 0x20; // bit 5
        } else {
            self.muteme_effect &= !0x20;
        }
        if self.kind == Some(DeviceKind::MuteMe) && self.color != [0, 0, 0] {
            self.set_color(self.color[0], self.color[1], self.color[2]);
        }
    }

    /// Enable or disable dim mode on the MuteMe device.
    /// Dim reduces LED brightness.
    #[allow(dead_code)]
    pub fn set_muteme_dim(&mut self, enabled: bool) {
        if enabled {
            self.muteme_effect |= 0x10; // bit 4
        } else {
            self.muteme_effect &= !0x10;
        }
        if self.kind == Some(DeviceKind::MuteMe) && self.color != [0, 0, 0] {
            self.set_color(self.color[0], self.color[1], self.color[2]);
        }
    }
}

impl Drop for Controller {
    fn drop(&mut self) {
        self.off();
    }
}
