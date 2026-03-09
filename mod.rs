
// Captures raw mouse button events using macOS IOHIDManager.
//
// SECURITY:
//   - We only read button number + press/release state
//   - No cursor position, no scroll data, no keyboard data captured
//   - Events are dispatched immediately and not stored anywhere
//   - unsafe blocks are minimal and clearly documented

use std::sync::Arc;

/// A mouse button event
#[derive(Debug, Clone)]
pub struct ButtonEvent {
    /// macOS HID button number (4 = first extra button, 5 = second, etc.)
    pub button: u32,
    /// true = pressed, false = released
    pub pressed: bool,
}

/// Callback type for button events
pub type ButtonCallback = Arc<dyn Fn(ButtonEvent) + Send + Sync + 'static>;

/// Start listening for HID mouse button events.
/// This function blocks forever (runs the CFRunLoop).
///
/// # Arguments
/// * `callback` - called on every button press/release. Executes on the main thread.
///
/// # Security
/// - Only button events are forwarded (usage page 0x09 = Button)
/// - No position/movement data is ever read or stored
pub fn start(callback: ButtonCallback) -> Result<(), HidError> {
    #[cfg(target_os = "macos")]
    {
        macos::run_hid_loop(callback)
    }
    #[cfg(not(target_os = "macos"))]
    {
        Err(HidError::UnsupportedPlatform)
    }
}

#[derive(Debug, thiserror::Error)]
pub enum HidError {
    #[error("Failed to create IOHIDManager — grant Input Monitoring permission in System Settings")]
    ManagerCreationFailed,
    #[error("This platform is not supported (macOS only)")]
    UnsupportedPlatform,
}

// ── macOS implementation ─────────────────────────────────────────────────────

#[cfg(target_os = "macos")]
mod macos {
    use super::{ButtonCallback, HidError};
    use core_foundation::runloop::CFRunLoop;

    // We use raw IOKit bindings via a small C shim embedded here.
    // All unsafe code is contained in this module only.
    mod ffi {
        #![allow(non_upper_case_globals, non_camel_case_types, dead_code)]

        use std::os::raw::{c_int, c_void};

        // Opaque types
        pub type IOHIDManagerRef = *mut c_void;
        pub type IOHIDValueRef   = *mut c_void;
        pub type IOHIDElementRef = *mut c_void;
        pub type CFRunLoopRef    = *mut c_void;
        pub type CFStringRef     = *const c_void;
        pub type CFDictionaryRef = *mut c_void;
        pub type CFNumberRef     = *mut c_void;
        pub type CFAllocatorRef  = *mut c_void;
        pub type IOReturn        = c_int;

        pub const kCFAllocatorDefault: CFAllocatorRef = std::ptr::null_mut();
        pub const kIOHIDOptionsTypeNone: u32 = 0;
        pub const kHIDPage_GenericDesktop: u32 = 0x01;
        pub const kHIDUsage_GD_Mouse: u32      = 0x02;
        pub const kHIDPage_Button: u32          = 0x09;

        pub type HIDInputCallback = unsafe extern "C" fn(
            context: *mut c_void,
            result: IOReturn,
            sender: *mut c_void,
            value: IOHIDValueRef,
        );

        #[link(name = "IOKit", kind = "framework")]
        #[link(name = "CoreFoundation", kind = "framework")]
        extern "C" {
            pub fn IOHIDManagerCreate(
                allocator: CFAllocatorRef,
                options: u32,
            ) -> IOHIDManagerRef;

            pub fn IOHIDManagerSetDeviceMatching(
                manager: IOHIDManagerRef,
                matching: CFDictionaryRef,
            );

            pub fn IOHIDManagerRegisterInputValueCallback(
                manager: IOHIDManagerRef,
                callback: HIDInputCallback,
                context: *mut c_void,
            );

            pub fn IOHIDManagerScheduleWithRunLoop(
                manager: IOHIDManagerRef,
                runLoop: CFRunLoopRef,
                runLoopMode: CFStringRef,
            );

            pub fn IOHIDManagerOpen(manager: IOHIDManagerRef, options: u32) -> IOReturn;

            pub fn IOHIDValueGetElement(value: IOHIDValueRef) -> IOHIDElementRef;
            pub fn IOHIDValueGetIntegerValue(value: IOHIDValueRef) -> isize;
            pub fn IOHIDElementGetUsagePage(element: IOHIDElementRef) -> u32;
            pub fn IOHIDElementGetUsage(element: IOHIDElementRef) -> u32;

            // CoreFoundation helpers
            pub fn CFDictionaryCreateMutable(
                allocator: CFAllocatorRef,
                capacity: isize,
                keyCallbacks: *const c_void,
                valueCallbacks: *const c_void,
            ) -> CFDictionaryRef;

            pub fn CFDictionarySetValue(
                dict: CFDictionaryRef,
                key: *const c_void,
                value: *const c_void,
            );

            pub fn CFNumberCreate(
                allocator: CFAllocatorRef,
                theType: c_int,
                valuePtr: *const c_void,
            ) -> CFNumberRef;

            pub fn CFRelease(cf: *const c_void);

            pub fn CFRunLoopGetCurrent() -> CFRunLoopRef;
            pub fn CFRunLoopRun();

            pub static kCFTypeDictionaryKeyCallBacks:   c_void;
            pub static kCFTypeDictionaryValueCallBacks: c_void;
            pub static kCFRunLoopDefaultMode:           c_void;
            pub static kIOHIDDeviceUsagePageKey:        c_void;
            pub static kIOHIDDeviceUsageKey:            c_void;
        }
    }

    /// Thread-local storage for the callback pointer during CFRunLoop execution.
    /// Security: This is never written to disk or sent anywhere.
    use std::cell::RefCell;
    thread_local! {
        static CALLBACK: RefCell<Option<ButtonCallback>> = RefCell::new(None);
    }

    /// The C-compatible callback invoked by IOHIDManager for every HID input value.
    ///
    /// # Safety
    /// Called by macOS on the CFRunLoop thread. We only read button page (0x09) events.
    unsafe extern "C" fn hid_input_callback(
        _context: *mut std::os::raw::c_void,
        _result: ffi::IOReturn,
        _sender: *mut std::os::raw::c_void,
        value: ffi::IOHIDValueRef,
    ) {
        // Safety: value is guaranteed non-null by IOHIDManager
        let element   = ffi::IOHIDValueGetElement(value);
        let usage_page = ffi::IOHIDElementGetUsagePage(element);
        let usage      = ffi::IOHIDElementGetUsage(element);

        // SECURITY: Only process Button page events (0x09)
        // We deliberately ignore mouse movement, scroll, and all other pages
        if usage_page != ffi::kHIDPage_Button {
            return;
        }

        // Only extra buttons (button 4+); buttons 1-3 are standard left/right/middle
        if usage < 4 {
            return;
        }

        let pressed = ffi::IOHIDValueGetIntegerValue(value) != 0;

        // Dispatch to Rust callback — event data is NOT stored
        CALLBACK.with(|cb| {
            if let Some(f) = cb.borrow().as_ref() {
                f(super::ButtonEvent { button: usage, pressed });
            }
        });
    }

    pub fn run_hid_loop(callback: ButtonCallback) -> Result<(), HidError> {
        // Store callback in thread-local for the C callback to access
        CALLBACK.with(|cb| {
            *cb.borrow_mut() = Some(callback);
        });

        unsafe {
            // Create IOHIDManager
            let manager = ffi::IOHIDManagerCreate(
                ffi::kCFAllocatorDefault,
                ffi::kIOHIDOptionsTypeNone,
            );
            if manager.is_null() {
                return Err(HidError::ManagerCreationFailed);
            }

            // Match only mice (GenericDesktop / Mouse)
            let dict = ffi::CFDictionaryCreateMutable(
                ffi::kCFAllocatorDefault, 2,
                &ffi::kCFTypeDictionaryKeyCallBacks as *const _ as *const _,
                &ffi::kCFTypeDictionaryValueCallBacks as *const _ as *const _,
            );
            let cf_int_type: std::os::raw::c_int = 9; // kCFNumberIntType
            let page_val = ffi::kHIDPage_GenericDesktop as std::os::raw::c_int;
            let usage_val = ffi::kHIDUsage_GD_Mouse as std::os::raw::c_int;
            let page_num  = ffi::CFNumberCreate(ffi::kCFAllocatorDefault, cf_int_type, &page_val  as *const _ as *const _);
            let usage_num = ffi::CFNumberCreate(ffi::kCFAllocatorDefault, cf_int_type, &usage_val as *const _ as *const _);

            ffi::CFDictionarySetValue(dict,
                &ffi::kIOHIDDeviceUsagePageKey as *const _ as *const _,
                page_num as *const _);
            ffi::CFDictionarySetValue(dict,
                &ffi::kIOHIDDeviceUsageKey as *const _ as *const _,
                usage_num as *const _);

            ffi::IOHIDManagerSetDeviceMatching(manager, dict);
            ffi::CFRelease(dict as *const _);
            ffi::CFRelease(page_num as *const _);
            ffi::CFRelease(usage_num as *const _);

            // Register our callback
            ffi::IOHIDManagerRegisterInputValueCallback(
                manager,
                hid_input_callback,
                std::ptr::null_mut(),
            );

            // Schedule on current run loop
            ffi::IOHIDManagerScheduleWithRunLoop(
                manager,
                ffi::CFRunLoopGetCurrent(),
                &ffi::kCFRunLoopDefaultMode as *const _ as *const _,
            );

            ffi::IOHIDManagerOpen(manager, ffi::kIOHIDOptionsTypeNone);

            log::info!("HID listener active — waiting for button events...");

            // Block forever on CFRunLoop
            ffi::CFRunLoopRun();
        }

        Ok(())
    }
}
