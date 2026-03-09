// Simulates keyboard shortcuts and macOS system actions via CGEventPost.
//
// SECURITY:
//   - We only POST events (output), never read events from other apps
//   - No screenshots, no clipboard access, no accessibility snooping
//   - All key codes come from a hardcoded allowlist — no arbitrary injection
//   - unsafe blocks are isolated here and clearly documented

#![cfg(target_os = "macos")]

use std::collections::HashMap;

/// Key code type alias (CGKeyCode = u16 on macOS)
type KeyCode = u16;

/// CGEventFlags bitmask
type EventFlags = u64;

const CMD:   EventFlags = 0x0010_0000;
const SHIFT: EventFlags = 0x0002_0000;
const ALT:   EventFlags = 0x0008_0000;
const CTRL:  EventFlags = 0x0004_0000;

mod ffi {
    #![allow(non_upper_case_globals, dead_code)]
    use std::os::raw::c_void;

    pub type CGKeyCode    = u16;
    pub type CGEventFlags = u64;
    pub type CGEventRef   = *mut c_void;
    pub type CGEventSourceRef = *mut c_void;

    pub const kCGEventSourceStateHIDSystemState: i32 = 1;
    pub const kCGHIDEventTap: u32 = 0;

    #[link(name = "CoreGraphics", kind = "framework")]
    extern "C" {
        pub fn CGEventSourceCreate(state: i32) -> CGEventSourceRef;
        pub fn CGEventCreateKeyboardEvent(
            source: CGEventSourceRef,
            keycode: CGKeyCode,
            keydown: bool,
        ) -> CGEventRef;
        pub fn CGEventSetFlags(event: CGEventRef, flags: CGEventFlags);
        pub fn CGEventPost(tap: u32, event: CGEventRef);
        pub fn CFRelease(cf: *const c_void);
    }
}

/// Simulate a key press + release with optional modifier flags.
///
/// # Safety
/// Uses CGEventPost — the standard macOS API for sending input events.
/// We never read other apps' events or memory.
fn post_key(keycode: KeyCode, modifiers: EventFlags) {
    unsafe {
        let src  = ffi::CGEventSourceCreate(ffi::kCGEventSourceStateHIDSystemState);
        let down = ffi::CGEventCreateKeyboardEvent(src, keycode, true);
        let up   = ffi::CGEventCreateKeyboardEvent(src, keycode, false);

        ffi::CGEventSetFlags(down, modifiers);
        ffi::CGEventSetFlags(up,   modifiers);

        ffi::CGEventPost(ffi::kCGHIDEventTap, down);
        ffi::CGEventPost(ffi::kCGHIDEventTap, up);

        ffi::CFRelease(down as *const _);
        ffi::CFRelease(up   as *const _);
        ffi::CFRelease(src  as *const _);
    }
}

// ── Public action functions ──────────────────────────────────────────────────

/// Trigger Mission Control (F3)
pub fn mission_control() {
    log::debug!("action: mission_control");
    post_key(0x63, 0); // kVK_F3
}

/// Trigger App Exposé — all windows of current app (Ctrl+F3)
pub fn expose_app() {
    log::debug!("action: expose_app");
    post_key(0x63, CTRL); // Ctrl+F3
}

/// Open the Cmd+Tab app switcher
pub fn app_switch() {
    log::debug!("action: app_switch");
    post_key(0x30, CMD); // Cmd+Tab
}

/// Cycle windows within current app (Cmd+`)
pub fn window_switch() {
    log::debug!("action: window_switch");
    post_key(0x32, CMD); // Cmd+`
}

/// Simulate a keyboard shortcut from a list of key name strings.
/// Keys must be from the hardcoded allowlist — no arbitrary strings executed.
pub fn shortcut(keys: &[String]) -> Result<(), ShortcutError> {
    let key_map   = build_key_map();
    let mod_map   = build_mod_map();

    let mut modifiers: EventFlags = 0;
    let mut keycode: Option<KeyCode> = None;

    for k in keys {
        let k = k.to_lowercase();
        if let Some(&flags) = mod_map.get(k.as_str()) {
            modifiers |= flags;
        } else if let Some(&code) = key_map.get(k.as_str()) {
            keycode = Some(code);
        } else {
            // This path is unreachable in practice because config validation
            // already checked the allowlist — but we handle it defensively
            return Err(ShortcutError::UnknownKey(k));
        }
    }

    let code = keycode.ok_or(ShortcutError::NoNonModifierKey)?;
    log::debug!("action: shortcut keys={:?} modifiers={:#x} keycode={:#x}", keys, modifiers, code);
    post_key(code, modifiers);
    Ok(())
}

#[derive(Debug, thiserror::Error)]
pub enum ShortcutError {
    #[error("Unknown key: '{0}'")]
    UnknownKey(String),
    #[error("Shortcut has no non-modifier key")]
    NoNonModifierKey,
}

// ── Key maps (allowlisted) ───────────────────────────────────────────────────

fn build_mod_map() -> HashMap<&'static str, EventFlags> {
    [
        ("cmd",     CMD),
        ("command", CMD),
        ("shift",   SHIFT),
        ("alt",     ALT),
        ("option",  ALT),
        ("ctrl",    CTRL),
        ("control", CTRL),
    ].into()
}

fn build_key_map() -> HashMap<&'static str, KeyCode> {
    [
        // Letters
        ("a",0x00),("s",0x01),("d",0x02),("f",0x03),("h",0x04),
        ("g",0x05),("z",0x06),("x",0x07),("c",0x08),("v",0x09),
        ("b",0x0B),("q",0x0C),("w",0x0D),("e",0x0E),("r",0x0F),
        ("y",0x10),("t",0x11),("1",0x12),("2",0x13),("3",0x14),
        ("4",0x15),("6",0x16),("5",0x17),("=",0x18),("9",0x19),
        ("7",0x1A),("-",0x1B),("8",0x1C),("0",0x1D),("]",0x1E),
        ("o",0x1F),("u",0x20),("[",0x21),("i",0x22),("p",0x23),
        ("l",0x25),("j",0x26),("'",0x27),("k",0x28),(";",0x29),
        ("\\",0x2A),(",",0x2B),("/",0x2C),("n",0x2D),("m",0x2E),
        (".",0x2F),("grave",0x32),("`",0x32),
        // Special
        ("space",0x31),("return",0x24),("enter",0x24),
        ("tab",0x30),("delete",0x33),("escape",0x35),("esc",0x35),
        // Arrows
        ("left",0x7B),("right",0x7C),("down",0x7D),("up",0x7E),
        ("home",0x73),("end",0x77),("pageup",0x74),("pagedown",0x79),
        // Function keys
        ("f1",0x7A),("f2",0x78),("f3",0x63),("f4",0x76),
        ("f5",0x60),("f6",0x61),("f7",0x62),("f8",0x64),
        ("f9",0x65),("f10",0x6D),("f11",0x67),("f12",0x6F),
    ].into()
}
