//! Self-update from GitHub releases, through fastframe-update.
//!
//! The previous release's helper installs the first release built on the
//! crate, so what it relies on stays the same: `--version` prints
//! `<command> <version>`, the new app accepts `--update-receipt` and
//! `--update-error` (stripped by [`fastframe_update::intercept`] before the
//! argument parser), and the handoff and receipt files keep their format.

pub use fastframe_update::{
    CHECK_INTERVAL, DownloadState, Installation, Kind, Prepared, Release, Source, Unsupported,
    Updater,
};
use fastframe_update::{MacConfig, ReqwestTransport, UpdateConfig};

pub const CONFIG: UpdateConfig = UpdateConfig {
    macos: MacConfig {
        bundle_ids: &["com.nickaustin.spindle"],
        executable_names: &["Spindle"],
        legacy_bundle_names: &[],
    },
    // Spindle's own releases live in Nick's repository. They are not signed
    // yet, so no publisher key: once a release workflow signs checksums.txt,
    // put its public key in assets/update-public-key.hex and require it here.
    // Spotifast's releases must never be offered: they would replace Spindle.
    publisher_key: None,
    // Spindle does not ship AppImages.
    appimage: false,
    ..UpdateConfig::new(
        "Nicolasaustin/spindle",
        "Spindle",
        "spindle",
        env!("CARGO_PKG_VERSION"),
    )
};

/// An updater on Spindle's HTTP client, through the configured proxy.
pub fn updater(proxy: &crate::settings::ProxyConfig) -> anyhow::Result<Updater> {
    let builder = crate::http::blocking_builder(proxy).map_err(anyhow::Error::msg)?;
    Ok(Updater::new(CONFIG, ReqwestTransport::new(builder)?))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn update_config_is_valid() {
        CONFIG.validate().unwrap();
        assert_eq!(CONFIG.current_version, env!("CARGO_PKG_VERSION"));
    }

    /// The updater reads Spindle's releases, never Spotifast's: a Spotifast
    /// release installed over Spindle would replace it.
    #[test]
    fn updates_come_from_spindles_own_repository() {
        assert_eq!(CONFIG.repository, "Nicolasaustin/spindle");
        assert_eq!(CONFIG.slug, "spindle");
    }
}
