mod audio_monitor;
mod busylight;
mod config;
mod icons;
mod sleep_monitor;
mod webhook;

use audio_monitor::AudioState;
use config::Config;
use crossbeam_channel::{unbounded, Receiver, Sender};
use icons::create_icon;
use log::{error, info};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant, SystemTime};
use tao::event_loop::{ControlFlow, EventLoopBuilder};
use tao::window::WindowBuilder;
use tray_icon::menu::{CheckMenuItem, Menu, MenuEvent, MenuItem, PredefinedMenuItem, Submenu};
use tray_icon::TrayIconBuilder;

struct App {
    config: Config,
    running: Arc<AtomicBool>,
    current_state: AudioState,
    busylight: busylight::Controller,
    last_tick: Instant,
    enable_item: CheckMenuItem,
    webhook_item: CheckMenuItem,
    webhook_enabled: Arc<AtomicBool>,
    status_item: MenuItem,
    quit_item: MenuItem,
    // Color preset check items
    busy_red: CheckMenuItem,
    busy_orange: CheckMenuItem,
    busy_yellow: CheckMenuItem,
    free_green: CheckMenuItem,
    free_blue: CheckMenuItem,
    free_cyan: CheckMenuItem,
    speaker_orange: CheckMenuItem,
    speaker_yellow: CheckMenuItem,
    speaker_purple: CheckMenuItem,
    // Poll interval items
    poll_500: CheckMenuItem,
    poll_1000: CheckMenuItem,
    poll_2000: CheckMenuItem,
    poll_3000: CheckMenuItem,
    // Calmdown items
    calm_off: CheckMenuItem,
    calm_1m: CheckMenuItem,
    calm_5m: CheckMenuItem,
    calm_15m: CheckMenuItem,
    calm_30m: CheckMenuItem,
}

fn main() {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info"))
        .format_timestamp_millis()
        .init();

    info!("Busy Me v{} starting...", env!("CARGO_PKG_VERSION"));

    let config = Config::load();
    let running = Arc::new(AtomicBool::new(true));
    let (audio_tx, audio_rx): (Sender<AudioState>, Receiver<AudioState>) = unbounded();
    let (webhook_tx, webhook_rx): (Sender<AudioState>, Receiver<AudioState>) = unbounded();

    // Icons
    let mut free_icon = create_icon(config.free_color);
    let mut busy_icon = create_icon(config.busy_color);
    let mut speaker_icon = create_icon(config.speaker_color);

    // Event loop
    let mut event_loop = EventLoopBuilder::new().build();

    #[cfg(target_os = "macos")]
    {
        use tao::platform::macos::{ActivationPolicy, EventLoopExtMacOS};
        event_loop.set_activation_policy(ActivationPolicy::Accessory);
    }

    // Register for macOS sleep notification (fires calmdown before suspend)
    let calmd_url = config.webhook_url_calmdown.clone();
    sleep_monitor::register(calmd_url);

    let _window = WindowBuilder::new()
        .with_title("Busy Me")
        .with_visible(false)
        .build(&event_loop)
        .unwrap();

    // ── Build tray menu ──

    let enable_item = CheckMenuItem::new("Enable Monitoring", true, config.enabled, None);
    let sep1 = PredefinedMenuItem::separator();

    // Busy color submenu (mic + cam → red)
    let busy_red = CheckMenuItem::new("Red", true, config.busy_color == [255, 0, 0], None);
    let busy_orange = CheckMenuItem::new("Orange", true, config.busy_color == [255, 120, 0], None);
    let busy_yellow = CheckMenuItem::new("Yellow", true, config.busy_color == [255, 200, 0], None);
    let busy_sub = Submenu::with_items("On Call (mic/cam) Color", true, &[&busy_red, &busy_orange, &busy_yellow]).unwrap();

    // Free color submenu
    let free_green = CheckMenuItem::new("Green", true, config.free_color == [40, 230, 40], None);
    let free_blue = CheckMenuItem::new("Blue", true, config.free_color == [40, 100, 255], None);
    let free_cyan = CheckMenuItem::new("Cyan", true, config.free_color == [40, 200, 200], None);
    let free_sub = Submenu::with_items("Free Color", true, &[&free_green, &free_blue, &free_cyan]).unwrap();

    // Speaker color submenu
    let speaker_orange = CheckMenuItem::new("Orange", true, config.speaker_color == [255, 160, 40], None);
    let speaker_yellow = CheckMenuItem::new("Yellow", true, config.speaker_color == [255, 200, 0], None);
    let speaker_purple = CheckMenuItem::new("Purple", true, config.speaker_color == [180, 60, 255], None);
    let speaker_sub = Submenu::with_items("Speaker Color", true, &[&speaker_orange, &speaker_yellow, &speaker_purple]).unwrap();

    let colors_sub = Submenu::with_items("Colors", true, &[&busy_sub, &free_sub, &speaker_sub]).unwrap();

    // Poll interval submenu
    let poll_500 = CheckMenuItem::new("0.5s", true, config.poll_interval_ms == 500, None);
    let poll_1000 = CheckMenuItem::new("1s", true, config.poll_interval_ms == 1000, None);
    let poll_2000 = CheckMenuItem::new("2s", true, config.poll_interval_ms == 2000, None);
    let poll_3000 = CheckMenuItem::new("3s", true, config.poll_interval_ms == 3000, None);
    let poll_sub = Submenu::with_items("Poll Interval", true, &[&poll_500, &poll_1000, &poll_2000, &poll_3000]).unwrap();

    let webhook_item = CheckMenuItem::new("Webhook → HA", true, config.webhook_enabled, None);

    // Calmdown timeout presets
    let calm_off = CheckMenuItem::new("Off", true, config.calmdown_secs == 0, None);
    let calm_1m = CheckMenuItem::new("1 min", true, config.calmdown_secs == 60, None);
    let calm_5m = CheckMenuItem::new("5 min", true, config.calmdown_secs == 300, None);
    let calm_15m = CheckMenuItem::new("15 min", true, config.calmdown_secs == 900, None);
    let calm_30m = CheckMenuItem::new("30 min", true, config.calmdown_secs == 1800, None);
    let calm_sub = Submenu::with_items("Calmdown Timer", true,
        &[&calm_off, &calm_1m, &calm_5m, &calm_15m, &calm_30m]).unwrap();

    let open_config_item = MenuItem::new("Open Config File...", true, None);
    let sep2 = PredefinedMenuItem::separator();
    let status_item = MenuItem::new("Status: waiting...", false, None);
    let sep3 = PredefinedMenuItem::separator();
    let quit_item = MenuItem::new("Quit", true, None);

    let menu = Menu::new();
    menu.append(&enable_item).unwrap();
    menu.append(&sep1).unwrap();
    menu.append(&colors_sub).unwrap();
    menu.append(&poll_sub).unwrap();
    menu.append(&webhook_item).unwrap();
    menu.append(&calm_sub).unwrap();
    menu.append(&open_config_item).unwrap();
    menu.append(&sep2).unwrap();
    menu.append(&status_item).unwrap();
    menu.append(&sep3).unwrap();
    menu.append(&quit_item).unwrap();

    // Tray icon
    let tray = TrayIconBuilder::new()
        .with_menu(Box::new(menu))
        .with_tooltip("Busy Me")
        .with_icon(free_icon.clone())
        .build()
        .unwrap();

    // ── Audio monitor thread ──
    let monitor_config = config.clone();
    audio_monitor::start_monitor(Arc::new(monitor_config), Arc::clone(&running), audio_tx);

    // ── Webhook thread ──
    let webhook_enabled = Arc::new(AtomicBool::new(config.webhook_enabled));
    webhook::start_webhook(
        Arc::from(config.webhook_url_free.clone()),
        Arc::from(config.webhook_url_speaker.clone()),
        Arc::from(config.webhook_url_busy.clone()),
        Arc::from(config.webhook_url_calmdown.clone()),
        Arc::clone(&webhook_enabled),
        config.calmdown_secs,
        webhook_rx,
        Arc::clone(&running),
    );

    // ── Sleep detection (dedicated thread, wall-clock) ──
    {
        let running = Arc::clone(&running);
        let wh_enabled = Arc::clone(&webhook_enabled);
        let url = config.webhook_url_calmdown.clone();
        std::thread::spawn(move || {
            let mut last = SystemTime::now();
            while running.load(Ordering::Relaxed) {
                std::thread::sleep(Duration::from_secs(1));
                let now = SystemTime::now();
                if let Ok(elapsed) = now.duration_since(last) {
                    if elapsed > Duration::from_secs(5)
                        && wh_enabled.load(Ordering::Relaxed)
                    {
                        webhook::fire_calmdown(&url);
                    }
                }
                last = now;
            }
        });
    }

    // ── Busylight ──
    let mut busylight = busylight::Controller::new();
    // Set initial color so the light reflects the current state immediately
    if busylight.is_connected() {
        busylight.set_color(config.free_color[0], config.free_color[1], config.free_color[2]);
    }
    let current_state = AudioState::Free;
    let last_tick = Instant::now();

    // ── App state ──
    let mut app = App {
        config,
        running,
        current_state,
        busylight,
        last_tick,
        enable_item,
        webhook_item,
        webhook_enabled,
        status_item,
        quit_item,
        busy_red,
        busy_orange,
        busy_yellow,
        free_green,
        free_blue,
        free_cyan,
        speaker_orange,
        speaker_yellow,
        speaker_purple,
        poll_500,
        poll_1000,
        poll_2000,
        poll_3000,
        calm_off,
        calm_1m,
        calm_5m,
        calm_15m,
        calm_30m,
    };

    let menu_channel = MenuEvent::receiver();

    event_loop.run(move |event, _elw, control_flow| {
        *control_flow = ControlFlow::WaitUntil(Instant::now() + Duration::from_millis(500));

        match event {
            tao::event::Event::NewEvents(_) => {
                // ── Audio state changes ──
                while let Ok(state) = audio_rx.try_recv() {
                    if state == app.current_state {
                        continue;
                    }
                    app.current_state = state;
                    // Forward to webhook thread (debounced)
                    let _ = webhook_tx.send(state);

                    let (icon, tip) = match state {
                        AudioState::Free => (&free_icon, "🟢 Free"),
                        AudioState::SpeakerActive => (&speaker_icon, "🟠 Speaker Active"),
                        AudioState::Busy => (&busy_icon, "🔴 Mic or Camera Active"),
                    };
                    tray.set_icon(Some(icon.clone()))
                        .unwrap_or_else(|e| error!("icon: {}", e));
                    tray.set_tooltip(Some(tip))
                        .unwrap_or_else(|e| error!("tooltip: {}", e));
                    app.status_item.set_text(format!("Status: {}", tip));

                    if app.busylight.is_connected() {
                        let (r, g, b) = match state {
                            AudioState::Free => (app.config.free_color[0], app.config.free_color[1], app.config.free_color[2]),
                            AudioState::SpeakerActive => (app.config.speaker_color[0], app.config.speaker_color[1], app.config.speaker_color[2]),
                            AudioState::Busy => (app.config.busy_color[0], app.config.busy_color[1], app.config.busy_color[2]),
                        };
                        app.busylight.set_muteme_blink(state == AudioState::Busy);
                        app.busylight.fade_to_color(r, g, b);
                    }
                }

                // ── Menu events ──
                while let Ok(menu_event) = menu_channel.try_recv() {
                    let id = menu_event.id();

                    if id == app.enable_item.id() {
                        app.config.enabled = app.enable_item.is_checked();
                        app.config.save();
                        continue;
                    }

                    if id == app.webhook_item.id() {
                        let enabled = app.webhook_item.is_checked();
                        app.webhook_enabled.store(enabled, Ordering::Relaxed);
                        app.config.webhook_enabled = enabled;
                        if enabled {
                            info!("Webhooks enabled — free → {}  speaker → {}  busy → {}",
                                  app.config.webhook_url_free,
                                  app.config.webhook_url_speaker,
                                  app.config.webhook_url_busy);
                        }
                        app.config.save();
                        continue;
                    }

                    if id == open_config_item.id() {
                        let path = Config::path();
                        info!("Opening config: {}", path.display());
                        #[cfg(target_os = "macos")]
                        std::process::Command::new("open").arg(&path).spawn().ok();
                        #[cfg(target_os = "windows")]
                        if let Some(s) = path.to_str() {
                            std::process::Command::new("cmd").args(["/c", "start", "", s]).spawn().ok();
                        }
                        continue;
                    }

                    if id == app.quit_item.id() {
                        app.running.store(false, Ordering::Relaxed);
                        *control_flow = ControlFlow::Exit;
                        continue;
                    }

                    // Busy color presets
                    if handle_color_check(&id, &app.busy_red, &app.busy_orange, &app.busy_yellow) {
                        let c = if id == app.busy_red.id() { [255, 0, 0] }
                                else if id == app.busy_orange.id() { [255, 120, 0] }
                                else { [255, 200, 0] };
                        app.config.busy_color = c;
                        app.config.save();
                        busy_icon = create_icon(c);
                        if app.current_state == AudioState::Busy {
                            tray.set_icon(Some(busy_icon.clone())).unwrap_or_else(|e| error!("icon: {}", e));
                        }
                        continue;
                    }

                    // Free color presets
                    if handle_color_check(&id, &app.free_green, &app.free_blue, &app.free_cyan) {
                        let c = if id == app.free_green.id() { [40, 230, 40] }
                                else if id == app.free_blue.id() { [40, 100, 255] }
                                else { [40, 200, 200] };
                        app.config.free_color = c;
                        app.config.save();
                        free_icon = create_icon(c);
                        if app.current_state == AudioState::Free {
                            tray.set_icon(Some(free_icon.clone())).unwrap_or_else(|e| error!("icon: {}", e));
                        }
                        continue;
                    }

                    // Speaker color presets
                    if handle_color_check(&id, &app.speaker_orange, &app.speaker_yellow, &app.speaker_purple) {
                        let c = if id == app.speaker_orange.id() { [255, 160, 40] }
                                else if id == app.speaker_yellow.id() { [255, 200, 0] }
                                else { [180, 60, 255] };
                        app.config.speaker_color = c;
                        app.config.save();
                        speaker_icon = create_icon(c);
                        if app.current_state == AudioState::SpeakerActive {
                            tray.set_icon(Some(speaker_icon.clone())).unwrap_or_else(|e| error!("icon: {}", e));
                        }
                        continue;
                    }

                    // Poll interval
                    let poll_items = [&app.poll_500, &app.poll_1000, &app.poll_2000, &app.poll_3000];
                    for item in &poll_items {
                        if id == item.id() {
                            for other in &poll_items {
                                other.set_checked(other.id() == id);
                            }
                            app.config.poll_interval_ms = if id == app.poll_500.id() { 500 }
                                else if id == app.poll_1000.id() { 1000 }
                                else if id == app.poll_2000.id() { 2000 }
                                else { 3000 };
                            app.config.save();
                            break;
                        }
                    }

                    // Calmdown timeout
                    let calm_items = [&app.calm_off, &app.calm_1m, &app.calm_5m, &app.calm_15m, &app.calm_30m];
                    for item in &calm_items {
                        if id == item.id() {
                            for other in &calm_items {
                                other.set_checked(other.id() == id);
                            }
                            app.config.calmdown_secs = if id == app.calm_off.id() { 0 }
                                else if id == app.calm_1m.id() { 60 }
                                else if id == app.calm_5m.id() { 300 }
                                else if id == app.calm_15m.id() { 900 }
                                else { 1800 };
                            app.config.save();
                            break;
                        }
                    }
                }

                // ── Busylight keepalive ──
                let now = Instant::now();
                if now.duration_since(app.last_tick) >= Duration::from_millis(500) {
                    app.busylight.tick();
                    app.last_tick = now;
                }
            }
            tao::event::Event::Suspended => {
                info!("System suspending — firing calmdown webhook");
                if app.config.webhook_enabled && app.config.calmdown_secs > 0 {
                    webhook::fire_calmdown(&app.config.webhook_url_calmdown);
                }
            }
            tao::event::Event::Resumed => {
                info!("System resumed from sleep");
            }
            tao::event::Event::LoopDestroyed => {
                if app.config.webhook_enabled && app.config.calmdown_secs > 0 {
                    webhook::fire_calmdown(&app.config.webhook_url_calmdown);
                }
                app.running.store(false, Ordering::Relaxed);
                app.busylight.off();
            }
            _ => {}
        }
    });
}

fn handle_color_check(
    id: &tray_icon::menu::MenuId,
    a: &CheckMenuItem,
    b: &CheckMenuItem,
    c: &CheckMenuItem,
) -> bool {
    if id == a.id() || id == b.id() || id == c.id() {
        a.set_checked(id == a.id());
        b.set_checked(id == b.id());
        c.set_checked(id == c.id());
        true
    } else {
        false
    }
}
