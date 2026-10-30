//! Tiny key/value persistence for high scores, coins and settings, plus
//! haptic feedback.
//!
//! Desktop builds write a text file in the platform's data directory.
//! Web builds call into `localStorage` / `navigator.vibrate` through the JS
//! plugin in `web/index.html`.

pub const SLOT_SKIN: u32 = 100;
pub const SLOT_MUTED: u32 = 101;
/// Coins banked across runs, spent on skins.
pub const SLOT_WALLET: u32 = 102;
/// Bitmask of unlocked skins.
pub const SLOT_UNLOCKED: u32 = 103;
/// Number of runs started, used to show control hints to new players.
pub const SLOT_RUNS: u32 = 104;

pub fn high_score_slot(mode_index: usize) -> u32 {
    mode_index as u32
}

#[cfg(target_arch = "wasm32")]
mod backend {
    extern "C" {
        fn rusty_scape_storage_get(slot: u32) -> f64;
        fn rusty_scape_storage_set(slot: u32, value: f64);
        fn rusty_scape_vibrate(ms: u32);
    }

    pub fn vibrate(ms: u32) {
        unsafe { rusty_scape_vibrate(ms) }
    }

    /// Lets the JS plugin loader confirm the Rust side matches.
    #[no_mangle]
    pub extern "C" fn rusty_scape_storage_crate_version() -> u32 {
        1
    }

    pub fn get(slot: u32) -> Option<f64> {
        let v = unsafe { rusty_scape_storage_get(slot) };
        if v.is_nan() {
            None
        } else {
            Some(v)
        }
    }

    pub fn set(slot: u32, value: f64) {
        unsafe { rusty_scape_storage_set(slot, value) }
    }
}

#[cfg(not(target_arch = "wasm32"))]
mod backend {
    use std::collections::BTreeMap;
    use std::path::PathBuf;

    fn save_path() -> Option<PathBuf> {
        let base = if cfg!(target_os = "windows") {
            std::env::var_os("APPDATA").map(PathBuf::from)
        } else if cfg!(target_os = "macos") {
            std::env::var_os("HOME").map(|h| PathBuf::from(h).join("Library/Application Support"))
        } else {
            std::env::var_os("XDG_DATA_HOME")
                .map(PathBuf::from)
                .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".local/share")))
        }?;
        Some(base.join("rusty-scape").join("save.txt"))
    }

    /// Desktop has no rumble; feedback comes from sound and screen shake.
    pub fn vibrate(_ms: u32) {}

    fn load_all() -> BTreeMap<u32, f64> {
        let Some(path) = save_path() else {
            return BTreeMap::new();
        };
        let Ok(text) = std::fs::read_to_string(path) else {
            return BTreeMap::new();
        };
        text.lines()
            .filter_map(|line| {
                let (k, v) = line.split_once('=')?;
                Some((k.trim().parse().ok()?, v.trim().parse().ok()?))
            })
            .collect()
    }

    pub fn get(slot: u32) -> Option<f64> {
        load_all().get(&slot).copied()
    }

    pub fn set(slot: u32, value: f64) {
        let Some(path) = save_path() else { return };
        let mut all = load_all();
        all.insert(slot, value);
        let text: String = all.iter().map(|(k, v)| format!("{k}={v}\n")).collect();
        if let Some(dir) = path.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        let _ = std::fs::write(path, text);
    }
}

pub use backend::{get, set, vibrate};
