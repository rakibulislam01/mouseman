// hid/mod.rs
//
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
        let _ = callback;
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
    use std::cell::RefCell;
    use std::os::raw::{c_int, c_void};

    mod ffi {
        #![allow(non_upper_case_globals, non_camel_case_types, dead_code)]
        use std::os::raw::{c_int, c_void};

        pub type IOHIDManagerRef        = *mut c_void;
        pub type IOHIDValueRef          = *mut c_void;
        pub type IOHIDElementRef        = *mut c_void;
        pub type CFRunLoopRef           = *mut c_void;
        pub type CFStringRef            = *const c_void;
        pub type CFDictionaryRef        = *mut c_void;
        pub type CFMutableDictionaryRef = *mut c_void;
        pub type CFNumberRef            = *mut c_void;
        pub type CFAllocatorRef         = *mut c_void;
        pub type IOReturn               = c_int;

        pub const K_CF_ALLOCATOR_DEFAULT:       CFAllocatorRef = std::ptr::null_mut();
        pub const K_IO_HID_OPTIONS_TYPE_NONE:   u32  = 0;
        pub const K_HID_PAGE_GENERIC_DESKTOP:   u32  = 0x01;
        pub const K_HID_USAGE_GD_MOUSE:         u32  = 0x02;
        pub const K_HID_PAGE_BUTTON:            u32  = 0x09;
        pub const K_CF_NUMBER_INT_TYPE:         c_int = 9;
        pub const K_CF_STRING_ENCODING_UTF8:    u32  = 0x0800_0100;

        pub type HIDInputCallback = unsafe extern "C" fn(
            context: *mut c_void,
            result: IOReturn,
            sender: *mut c_void,
            value: IOHIDValueRef,
        );

        #[link(name = "IOKit", kind = "framework")]
        #[link(name = "CoreFoundation", kind = "framework")]
        extern "C" {
            pub fn IOHIDManagerCreate(allocator: CFAllocatorRef, options: u32) -> IOHIDManagerRef;
            pub fn IOHIDManagerSetDeviceMatching(mgr: IOHIDManagerRef, matching: CFDictionaryRef);
            pub fn IOHIDManagerRegisterInputValueCallback(
                mgr: IOHIDManagerRef,
                callback: HIDInputCallback,
                context: *mut c_void,
            );
            pub fn IOHIDManagerScheduleWithRunLoop(
                mgr: IOHIDManagerRef,
                run_loop: CFRunLoopRef,
                mode: CFStringRef,
            );
            pub fn IOHIDManagerOpen(mgr: IOHIDManagerRef, options: u32) -> IOReturn;

            pub fn IOHIDValueGetElement(value: IOHIDValueRef) -> IOHIDElementRef;
            pub fn IOHIDValueGetIntegerValue(value: IOHIDValueRef) -> isize;
            pub fn IOHIDElementGetUsagePage(element: IOHIDElementRef) -> u32;
            pub fn IOHIDElementGetUsage(element: IOHIDElementRef) -> u32;

            pub fn CFDictionaryCreateMutable(
                allocator: CFAllocatorRef,
                capacity: isize,
                key_cbs: *const c_void,
                val_cbs: *const c_void,
            ) -> CFMutableDictionaryRef;
            pub fn CFDictionarySetValue(
                dict: CFMutableDictionaryRef,
                key: *const c_void,
                val: *const c_void,
            );
            pub fn CFNumberCreate(
                allocator: CFAllocatorRef,
                kind: c_int,
                val: *const c_void,
            ) -> CFNumberRef;
            pub fn CFStringCreateWithCString(
                allocator: CFAllocatorRef,
                c_str: *const std::os::raw::c_char,
                encoding: u32,
            ) -> CFStringRef;
            pub fn CFRelease(cf: *const c_void);
            pub fn CFRunLoopGetCurrent() -> CFRunLoopRef;
            pub fn CFRunLoopRun();

            // These ARE proper linkable symbols (global variables, not string macros)
            pub static kCFTypeDictionaryKeyCallBacks:   c_void;
            pub static kCFTypeDictionaryValueCallBacks: c_void;
            pub static kCFRunLoopDefaultMode:           c_void;
        }
    }

    // IOKit key names — these are #define string macros in IOKit headers,
    // so we use their string values directly rather than trying to link them.
    const USAGE_PAGE_KEY: &[u8] = b"DeviceUsagePage\0";
    const USAGE_KEY:      &[u8] = b"DeviceUsage\0";

    thread_local! {
        static CALLBACK: RefCell<Option<ButtonCallback>> = RefCell::new(None);
    }

    /// IOHIDManager input callback — invoked by macOS for every HID value change.
    ///
    /// # Safety
    /// `value` is guaranteed non-null by IOHIDManager.
    /// We only read button-page events and do not store any data.
    unsafe extern "C" fn hid_input_callback(
        _context: *mut c_void,
        _result:  ffi::IOReturn,
        _sender:  *mut c_void,
        value:    ffi::IOHIDValueRef,
    ) {
        let element    = ffi::IOHIDValueGetElement(value);
        let usage_page = ffi::IOHIDElementGetUsagePage(element);
        let usage      = ffi::IOHIDElementGetUsage(element);

        // SECURITY: Only process Button page (0x09)
        // Ignores mouse movement, scroll wheel, and all other HID pages
        if usage_page != ffi::K_HID_PAGE_BUTTON {
            return;
        }
        // Skip standard left / middle / right click (buttons 1–3)
        if usage < 4 {
            return;
        }

        let pressed = ffi::IOHIDValueGetIntegerValue(value) != 0;

        // Dispatch immediately — data is NOT retained after this call
        CALLBACK.with(|cb| {
            if let Some(f) = cb.borrow().as_ref() {
                f(super::ButtonEvent { button: usage, pressed });
            }
        });
    }

    pub fn run_hid_loop(callback: ButtonCallback) -> Result<(), HidError> {
        CALLBACK.with(|cb| *cb.borrow_mut() = Some(callback));

        unsafe {
            // Create IOHIDManager
            let manager = ffi::IOHIDManagerCreate(
                ffi::K_CF_ALLOCATOR_DEFAULT,
                ffi::K_IO_HID_OPTIONS_TYPE_NONE,
            );
            if manager.is_null() {
                return Err(HidError::ManagerCreationFailed);
            }

            // Build matching dictionary: GenericDesktop page, Mouse usage
            let dict = ffi::CFDictionaryCreateMutable(
                ffi::K_CF_ALLOCATOR_DEFAULT,
                2,
                &ffi::kCFTypeDictionaryKeyCallBacks as *const _ as *const c_void,
                &ffi::kCFTypeDictionaryValueCallBacks as *const _ as *const c_void,
            );

            let page_val:  c_int = ffi::K_HID_PAGE_GENERIC_DESKTOP as c_int;
            let usage_val: c_int = ffi::K_HID_USAGE_GD_MOUSE as c_int;

            let page_num = ffi::CFNumberCreate(
                ffi::K_CF_ALLOCATOR_DEFAULT,
                ffi::K_CF_NUMBER_INT_TYPE,
                &page_val as *const _ as *const c_void,
            );
            let usage_num = ffi::CFNumberCreate(
                ffi::K_CF_ALLOCATOR_DEFAULT,
                ffi::K_CF_NUMBER_INT_TYPE,
                &usage_val as *const _ as *const c_void,
            );

            // Build CFString keys from CStr literals — avoids unresolvable IOKit string macros
            let page_key = ffi::CFStringCreateWithCString(
                ffi::K_CF_ALLOCATOR_DEFAULT,
                USAGE_PAGE_KEY.as_ptr() as *const std::os::raw::c_char,
                ffi::K_CF_STRING_ENCODING_UTF8,
            );
            let usage_key = ffi::CFStringCreateWithCString(
                ffi::K_CF_ALLOCATOR_DEFAULT,
                USAGE_KEY.as_ptr() as *const std::os::raw::c_char,
                ffi::K_CF_STRING_ENCODING_UTF8,
            );

            ffi::CFDictionarySetValue(dict, page_key  as *const c_void, page_num  as *const c_void);
            ffi::CFDictionarySetValue(dict, usage_key as *const c_void, usage_num as *const c_void);

            ffi::CFRelease(page_key  as *const c_void);
            ffi::CFRelease(usage_key as *const c_void);
            ffi::CFRelease(page_num  as *const c_void);
            ffi::CFRelease(usage_num as *const c_void);

            ffi::IOHIDManagerSetDeviceMatching(manager, dict);
            ffi::CFRelease(dict as *const c_void);

            // Register our callback and open the manager
            ffi::IOHIDManagerRegisterInputValueCallback(
                manager,
                hid_input_callback,
                std::ptr::null_mut(),
            );
            ffi::IOHIDManagerScheduleWithRunLoop(
                manager,
                ffi::CFRunLoopGetCurrent(),
                &ffi::kCFRunLoopDefaultMode as *const _ as *const c_void,
            );
            ffi::IOHIDManagerOpen(manager, ffi::K_IO_HID_OPTIONS_TYPE_NONE);

            log::info!("HID listener active — waiting for button events...");

            // Blocks here forever — macOS delivers HID events via this run loop
            ffi::CFRunLoopRun();
        }

        Ok(())
    }
}
