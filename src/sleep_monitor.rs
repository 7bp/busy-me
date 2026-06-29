/// Registers a handler for macOS sleep notifications via NSWorkspace.
/// Fires the calmdown webhook *before* the system suspends, while the
/// network stack is still operational.
#[cfg(target_os = "macos")]
use log::info;

#[cfg(target_os = "macos")]
pub fn register(calmdown_url: String) {
    use block::ConcreteBlock;
    use objc::runtime::Object;
    use objc::*;

    unsafe {
        let workspace: *mut Object = msg_send![class!(NSWorkspace), sharedWorkspace];
        let nc: *mut Object = msg_send![workspace, notificationCenter];

        // Build NSString via raw msg_send (avoids cocoa-foundation deprecation)
        let cls: *mut Object = msg_send![class!(NSString), alloc];
        let name: *mut Object = msg_send![cls, initWithUTF8String:
            b"NSWorkspaceWillSleepNotification\0".as_ptr() as *const std::ffi::c_char];

        // nil queue = deliver on the caller's run loop (which will be the
        // main event loop's CFRunLoop once event_loop.run() starts).
        let null_queue: *mut Object = std::ptr::null_mut();

        let url = calmdown_url.clone();
        let handler = ConcreteBlock::new(move |_: &Object| {
            info!("NSWorkspaceWillSleepNotification — firing calmdown");
            crate::webhook::fire_calmdown(&url);
        });
        let handler = handler.copy();

        let () = msg_send![nc,
            addObserverForName: name
            object: std::ptr::null_mut::<Object>()
            queue: null_queue
            usingBlock: &*handler
        ];

        // Block must live for the app's lifetime; leak it deliberately.
        std::mem::forget(handler);
    }
}

#[cfg(not(target_os = "macos"))]
pub fn register(_calmdown_url: String) {}
