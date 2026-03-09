// mouseman — open-source mouse button remapper for macOS
//
// SECURITY GUARANTEES:
//   ✓ No network access — zero outbound connections, ever
//   ✓ No data collection — button events processed in-memory, immediately discarded
//   ✓ No telemetry — no analytics, no crash reporting, no pings
//   ✓ No persistence — nothing written to disk at runtime except what YOU configure
//   ✓ Memory safe — Rust ownership prevents buffer overflows & use-after-free
//   ✓ Minimal unsafe — all unsafe code isolated in hid/ and macos/ with clear comments
//   ✓ Input validation — config keys validated against allowlist before use
//   ✓ Open source — every line is auditable

mod actions;
mod config;
mod hid;

#[cfg(target_os = "macos")]
mod macos;

use std::path::PathBuf;
use std::sync::Arc;
use clap::Parser;

const VERSION: &str = env!("CARGO_PKG_VERSION");

#[derive(Parser, Debug)]
#[command(
    name = "mouseman",
    version = VERSION,
    about = "Open-source mouse button remapper for macOS — no telemetry, no data collection",
    long_about = None,
)]
struct Args {
    /// Path to config YAML file
    #[arg(
        short, long,
        default_value_os_t = default_config_path(),
        help = "Path to config.yaml"
    )]
    config: PathBuf,

    /// Enable verbose debug logging
    #[arg(short, long)]
    verbose: bool,
}

fn default_config_path() -> PathBuf {
    dirs::home_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join(".config/mouseman/config.yaml")
}

fn main() {
    let args = Args::parse();

    // Init logging — outputs to stderr only, never to disk or network
    let log_level = if args.verbose { "debug" } else { "info" };
    env_logger::Builder::new()
        .filter_level(log_level.parse().unwrap())
        .format_timestamp(None)
        .init();

    println!("mouseman v{VERSION} — open-source mouse button remapper for macOS");
    println!("Config: {}", args.config.display());
    println!();

    // Load and validate config
    let config = match config::Config::load(&args.config) {
        Ok(cfg) => cfg,
        Err(e) => {
            eprintln!("❌ Config error: {e}");
            eprintln!();
            eprintln!("Create a config file at: {}", args.config.display());
            eprintln!("Or pass a custom path: mouseman --config ~/my-config.yaml");
            std::process::exit(1);
        }
    };

    // Print loaded mappings
    println!("Loaded {} button mapping(s):", config.buttons.len());
    for (btn, action) in &config.buttons {
        if action.keys.is_empty() {
            println!("  {btn} → {:?}", action.action);
        } else {
            println!("  {btn} → {:?}  keys={:?}", action.action, action.keys);
        }
    }
    println!();

    // Permission reminder
    println!("⚠️  Required macOS permissions (grant in System Settings):");
    println!("   Privacy & Security → Input Monitoring → add mouseman");
    println!("   Privacy & Security → Accessibility    → add mouseman");
    println!();

    // Build executor
    let executor = Arc::new(actions::Executor::new(config));

    // Start HID listener — blocks forever
    log::info!("Starting HID listener...");
    let cb_executor = Arc::clone(&executor);
    if let Err(e) = hid::start(Arc::new(move |event| {
        cb_executor.handle(event.button, event.pressed);
    })) {
        eprintln!("❌ Failed to start HID listener: {e}");
        std::process::exit(1);
    }
}
