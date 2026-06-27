use crossbeam_channel::Sender;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::Duration;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AudioState {
    Free,
    SpeakerActive,
    Busy, // mic OR webcam active
}

pub fn start_monitor(
    config: Arc<crate::config::Config>,
    running: Arc<AtomicBool>,
    tx: Sender<AudioState>,
) {
    thread::spawn(move || {
        let mut last_state = AudioState::Free;

        while running.load(Ordering::Relaxed) {
            if config.enabled {
                let mic = is_mic_busy();
                let speaker = is_speaker_busy();
                let cam = is_camera_busy();

                let state = if mic || cam {
                    AudioState::Busy
                } else if speaker {
                    AudioState::SpeakerActive
                } else {
                    AudioState::Free
                };

                if state != last_state {
                    last_state = state;
                    let _ = tx.send(state);
                }
            }

            thread::sleep(Duration::from_millis(config.poll_interval_ms));
        }
    });
}

// ──────────────────────────────────────────────
// macOS
// ──────────────────────────────────────────────

#[cfg(target_os = "macos")]
mod platform {
    use coreaudio_sys::*;
    use std::ffi::c_void;
    use std::mem;
    use std::ptr;

    // ── Audio (CoreAudio) ──

    fn default_device(selector: AudioObjectPropertySelector) -> Option<u32> {
        unsafe {
            let address = AudioObjectPropertyAddress {
                mSelector: selector,
                mScope: kAudioObjectPropertyScopeGlobal,
                mElement: kAudioObjectPropertyElementMain,
            };
            let mut device_id: AudioDeviceID = 0;
            let mut size = mem::size_of::<AudioDeviceID>() as u32;
            let status = AudioObjectGetPropertyData(
                kAudioObjectSystemObject,
                &address,
                0,
                ptr::null(),
                &mut size,
                &mut device_id as *mut _ as *mut c_void,
            );
            if status == 0 { Some(device_id) } else { None }
        }
    }

    fn device_is_running(device_id: u32) -> bool {
        unsafe {
            let address = AudioObjectPropertyAddress {
                mSelector: kAudioDevicePropertyDeviceIsRunningSomewhere,
                mScope: kAudioObjectPropertyScopeGlobal,
                mElement: kAudioObjectPropertyElementMain,
            };
            let mut running: u32 = 0;
            let mut size = mem::size_of::<u32>() as u32;
            let status = AudioObjectGetPropertyData(
                device_id,
                &address,
                0,
                ptr::null(),
                &mut size,
                &mut running as *mut _ as *mut c_void,
            );
            status == 0 && running != 0
        }
    }

    pub fn is_mic_busy() -> bool {
        default_device(kAudioHardwarePropertyDefaultInputDevice).map_or(false, device_is_running)
    }

    pub fn is_speaker_busy() -> bool {
        default_device(kAudioHardwarePropertyDefaultOutputDevice).map_or(false, device_is_running)
    }

    // ── Webcam (CoreMediaIO) ──

    /// CoreMediaIO property constants (from CMIOHardwareSystem.h / CMIOHardwareDevice.h)
    const CMIO_SYSTEM_OBJECT: u32 = 1;
    const CMIO_PROP_DEFAULT_INPUT: u32 = 0x64496E20; // 'dIn '
    const CMIO_PROP_DEVICE_RUNNING: u32 = 0x676F6E65; // 'gone'
    const CMIO_SCOPE_GLOBAL: u32 = 0x676C6F62; // 'glob'
    const CMIO_ELEMENT_MAIN: u32 = 0;

    type CMIOObjectID = u32;
    type OSStatus = i32;

    #[repr(C)]
    #[allow(non_snake_case)]
    struct CMIOObjectPropertyAddress {
        mSelector: u32,
        mScope: u32,
        mElement: u32,
    }

    #[link(name = "CoreMediaIO", kind = "framework")]
    extern "C" {
        // CMIOObjectGetPropertyData has 7 params (unlike CoreAudio's 6):
        //   (id, addr, qualSize, qualData, dataSize, *dataUsed, *outData)
        fn CMIOObjectGetPropertyData(
            object: CMIOObjectID,
            address: *const CMIOObjectPropertyAddress,
            qualifier_data_size: u32,
            qualifier_data: *const std::ffi::c_void,
            data_size: u32,
            data_used: *mut u32,
            data: *mut std::ffi::c_void,
        ) -> OSStatus;
    }

    /// Check if any process is actively using a camera device via CoreMediaIO.
    pub fn is_camera_busy() -> bool {
        unsafe {
            // 1. Get default camera device
            let address = CMIOObjectPropertyAddress {
                mSelector: CMIO_PROP_DEFAULT_INPUT,
                mScope: CMIO_SCOPE_GLOBAL,
                mElement: CMIO_ELEMENT_MAIN,
            };
            let mut device_id: CMIOObjectID = 0;
            let mut data_used: u32 = 0;

            let status = CMIOObjectGetPropertyData(
                CMIO_SYSTEM_OBJECT,
                &address,
                0,
                std::ptr::null(),
                std::mem::size_of::<CMIOObjectID>() as u32,
                &mut data_used,
                &mut device_id as *mut _ as *mut std::ffi::c_void,
            );
            if status != 0 || device_id == 0 {
                return false;
            }

            // 2. Check if the device is currently streaming
            let running_addr = CMIOObjectPropertyAddress {
                mSelector: CMIO_PROP_DEVICE_RUNNING,
                mScope: CMIO_SCOPE_GLOBAL,
                mElement: CMIO_ELEMENT_MAIN,
            };
            let mut running: u32 = 0;
            data_used = 0;

            let status = CMIOObjectGetPropertyData(
                device_id,
                &running_addr,
                0,
                std::ptr::null(),
                std::mem::size_of::<u32>() as u32,
                &mut data_used,
                &mut running as *mut _ as *mut std::ffi::c_void,
            );
            status == 0 && running != 0
        }
    }
}

// ──────────────────────────────────────────────
// Windows: WASAPI audio sessions + DeviceInformation
// ──────────────────────────────────────────────

#[cfg(target_os = "windows")]
mod platform {
    use windows::Devices::Enumeration::*;
    use windows::Win32::Media::Audio::*;
    use windows::Win32::System::Com::*;

    fn session_count(data_flow: EDataFlow) -> i32 {
        unsafe {
            let _ = CoInitializeEx(None, COINIT_MULTITHREADED);

            let enumerator: IMMDeviceEnumerator = match CoCreateInstance(
                &MMDeviceEnumerator,
                None,
                CLSCTX_ALL,
            ) {
                Ok(e) => e,
                Err(_) => return 0,
            };

            let device = match enumerator.GetDefaultAudioEndpoint(data_flow, eConsole) {
                Ok(d) => d,
                Err(_) => return 0,
            };

            let manager: IAudioSessionManager2 = match device.Activate(CLSCTX_ALL, None) {
                Ok(m) => m,
                Err(_) => return 0,
            };

            let sessions = match manager.GetSessionEnumerator() {
                Ok(s) => s,
                Err(_) => return 0,
            };

            sessions.GetCount().unwrap_or(0)
        }
    }

    pub fn is_mic_busy() -> bool {
        session_count(eCapture) > 0
    }

    pub fn is_speaker_busy() -> bool {
        // System sounds always have ≥1 session on render;
        // >1 means a non-system app is actively using speakers.
        session_count(eRender) > 0
    }

    pub fn is_camera_busy() -> bool {
        // DeviceAccessInformation tells us if a device class is available
        // vs. denied-by-system (in use by another app) or denied-by-user.
        match DeviceAccessInformation::CreateFromDeviceClass(DeviceClass::VideoCapture) {
            Ok(access) => match access.CurrentStatus() {
                Ok(status) => status == DeviceAccessStatus::DeniedBySystem,
                Err(_) => false,
            },
            Err(_) => false,
        }
    }
}

#[cfg(not(any(target_os = "macos", target_os = "windows")))]
mod platform {
    pub fn is_mic_busy() -> bool { false }
    pub fn is_speaker_busy() -> bool { false }
    pub fn is_camera_busy() -> bool { false }
}

pub use platform::*;
