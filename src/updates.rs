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
use fastframe_update::{MacConfig, MacTarget, ReqwestTransport, UpdateConfig};
use std::sync::Mutex;

type UpdaterCache = Mutex<Option<(crate::settings::ProxyConfig, Updater)>>;
static UPDATER: UpdaterCache = Mutex::new(None);

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
    // Spindle does not ship AppImages, and its Mac build is Apple silicon
    // only: the release carries spindle-v<version>-macos-arm64.dmg.
    appimage: false,
    mac_target: MacTarget::Arm64Only,
    ..UpdateConfig::new(
        "Limpingg/spindle",
        "Spindle",
        "spindle",
        env!("CARGO_PKG_VERSION"),
    )
};

/// An updater on Spindle's HTTP client, through the configured proxy.
pub fn updater(proxy: &crate::settings::ProxyConfig) -> anyhow::Result<Updater> {
    cached_updater(&UPDATER, proxy, || {
        let builder = crate::http::blocking_builder(proxy).map_err(anyhow::Error::msg)?;
        Ok(Updater::new(CONFIG, ReqwestTransport::new(builder)?))
    })
}

/// Rebuilds the updater on its next use after proxy settings are reapplied.
pub fn forget_updater() {
    forget_cached_updater(&UPDATER);
}

fn forget_cached_updater(cache: &UpdaterCache) {
    *cache.lock().unwrap_or_else(|lock| lock.into_inner()) = None;
}

// Check, download, and handoff share the HTTP runtime, certificate store,
// and connection pool. A changed proxy policy discards the previous client;
// a failed replacement must never fall back to that client's route.
fn cached_updater(
    cache: &UpdaterCache,
    proxy: &crate::settings::ProxyConfig,
    create: impl FnOnce() -> anyhow::Result<Updater>,
) -> anyhow::Result<Updater> {
    let mut cached = cache.lock().unwrap_or_else(|lock| lock.into_inner());
    if let Some((policy, updater)) = cached.as_ref()
        && policy == proxy
    {
        return Ok(updater.clone());
    }
    *cached = None;
    let updater = create()?;
    *cached = Some((proxy.clone(), updater.clone()));
    Ok(updater)
}

#[cfg(test)]
mod tests {
    use super::*;

    struct NoNetwork;

    impl fastframe_update::Transport for NoNetwork {
        fn get(
            &self,
            _: &fastframe_update::Request<'_>,
        ) -> anyhow::Result<fastframe_update::Response> {
            panic!("cache tests must never send a request")
        }
    }

    #[test]
    fn update_operations_reuse_the_client_for_the_same_proxy() {
        let cache = Mutex::new(None);
        let proxy = crate::settings::ProxyConfig::Off;
        let first = cached_updater(&cache, &proxy, || Ok(Updater::new(CONFIG, NoNetwork))).unwrap();
        let second = cached_updater(&cache, &proxy, || panic!("client was rebuilt")).unwrap();
        // Setting a test feed affects only the returned clone, not the cache.
        let local = Source::local("http://127.0.0.1:8123/").unwrap();
        assert_eq!(first.with_source(local.clone()).source(), &local);
        assert!(second.source().is_github());
    }

    #[test]
    fn proxy_changes_replace_the_client_and_failures_do_not_reuse_it() {
        let cache = Mutex::new(None);
        let off = crate::settings::ProxyConfig::Off;
        let system = crate::settings::ProxyConfig::System;
        cached_updater(&cache, &off, || Ok(Updater::new(CONFIG, NoNetwork))).unwrap();
        let error = cached_updater(&cache, &system, || anyhow::bail!("invalid proxy")).unwrap_err();
        assert_eq!(error.to_string(), "invalid proxy");
        assert!(cache.lock().unwrap().is_none());
        cached_updater(&cache, &system, || Ok(Updater::new(CONFIG, NoNetwork))).unwrap();
        cached_updater(&cache, &system, || panic!("replacement was not cached")).unwrap();
    }

    #[test]
    fn reapplying_system_proxy_rebuilds_the_cached_client() {
        let cache = Mutex::new(None);
        let proxy = crate::settings::ProxyConfig::System;
        cached_updater(&cache, &proxy, || Ok(Updater::new(CONFIG, NoNetwork))).unwrap();
        forget_cached_updater(&cache);
        assert!(cache.lock().unwrap().is_none());
        let mut rebuilt = false;
        cached_updater(&cache, &proxy, || {
            rebuilt = true;
            Ok(Updater::new(CONFIG, NoNetwork))
        })
        .unwrap();
        assert!(
            rebuilt,
            "the same policy must resolve the system proxy again"
        );
        cached_updater(&cache, &proxy, || panic!("replacement was not cached")).unwrap();
    }

    #[test]
    fn update_config_is_valid() {
        CONFIG.validate().unwrap();
        assert_eq!(CONFIG.current_version, env!("CARGO_PKG_VERSION"));
    }

    /// The updater reads Spindle's releases, never Spotifast's: a Spotifast
    /// release installed over Spindle would replace it.
    #[test]
    fn updates_come_from_spindles_own_repository() {
        assert_eq!(CONFIG.repository, "Limpingg/spindle");
        assert_eq!(CONFIG.slug, "spindle");
        assert!(matches!(CONFIG.mac_target, MacTarget::Arm64Only));
    }
}
