# mouseman 🖱️

Open-source mouse button remapper for macOS. Built in Rust.

Works with any USB/Bluetooth mouse (A4tech, generic, off-brand) — no vendor software needed.

## Security Guarantees

- 🔒 **No network access** — zero outbound connections, ever. No telemetry, no pings.
- 🔒 **No data collection** — button events processed in memory and immediately discarded
- 🔒 **No persistence** — nothing written to disk at runtime
- 🔒 **Memory safe** — Rust prevents buffer overflows, use-after-free, data races by default
- 🔒 **Minimal unsafe** — all `unsafe` blocks isolated in `hid/` and `macos/` with comments
- 🔒 **Input validation** — config keys validated against a hardcoded allowlist
- 🔒 **Open source** — every line is auditable

## Features (v1)

Remap extra mouse buttons (button4, button5, ...) to:

- **Mission Control** — see all desktops and spaces
- **App Switcher** — Cmd+Tab overlay
- **Window Switch** — cycle windows of current app (Cmd+`)
- **App Exposé** — show all windows of current app
- **Custom keyboard shortcuts** — any key combo from the allowlist

## Requirements

- macOS 13 Ventura or later (including Tahoe / macOS 16)
- Rust 1.75+ (`curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh`)
- Xcode Command Line Tools: `xcode-select --install`

## Build & Install

```bash
git clone https://github.com/yourusername/mouseman
cd mouseman
make install
```

## Setup

```bash
mkdir -p ~/.config/mouseman
cp config.yaml ~/.config/mouseman/config.yaml
# Edit the config to your liking
nano ~/.config/mouseman/config.yaml
```

## Run

```bash
mouseman
# With a custom config path:
mouseman --config ~/my-mouse.yaml
# Debug mode (shows button numbers from your mouse):
mouseman --verbose
```

## macOS Permissions

On first run, grant both permissions in **System Settings → Privacy & Security**:

1. **Input Monitoring** → add `mouseman`
2. **Accessibility** → add `mouseman`

Then restart mouseman.

## Config Reference

```yaml
buttons:
  button4:
    action: app_switch        # Cmd+Tab app switcher

  button5:
    action: mission_control   # Mission Control

  button6:
    action: window_switch     # Cmd+` (cycle app windows)

  button7:
    action: expose_app        # App Exposé

  button8:
    action: shortcut
    keys: ["cmd", "shift", "4"]  # Custom shortcut — screenshot
```

### Available actions

| Action | Description |
|---|---|
| `mission_control` | Open Mission Control (all spaces) |
| `app_switch` | Open Cmd+Tab app switcher |
| `window_switch` | Cycle windows of current app (Cmd+\`) |
| `expose_app` | App Exposé — all windows of current app |
| `shortcut` | Simulate a keyboard shortcut (requires `keys:`) |

### Finding your button numbers

Run `mouseman --verbose` and press your extra buttons — you'll see which number each one is.

## How It Works

- **IOHIDManager** (via Rust FFI) captures raw mouse button events from macOS
- **CGEventPost** (via Rust FFI) simulates keyboard shortcuts
- Config is a simple YAML file — no GUI, no background service, no daemon installer
- Runs as a foreground process; add to Login Items if you want it to auto-start

## License

MIT
