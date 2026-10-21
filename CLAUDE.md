# Spindle (fork of Spotifast)

This tree is crmne/spotifast 0.14.0 (MIT), renamed to Spindle for Nick. The
upstream remote is `upstream`. AGENTS.md is spotifast's own agent guide and
still applies to the code; this file adds what is different here.

Writing rule from Nick: no em dashes or en dashes anywhere, ever.

## What changed from upstream

- `/home/claude/rename-fork.sh` did the rename: Spotifast -> Spindle,
  spotifast -> spindle, bundle id com.nickaustin.spindle, bin src/spindle.rs,
  assets/skins/spindle.wsz, contrib/omarchy/spindle*. Upstream links
  (spotifast.rocks, github.com/crmne/spotifast) were restored on purpose.
- Settings default `check_for_updates: false` (the updater would fetch
  spotifast releases).
- About shows "Spindle is a fork of Spotifast by Carmine Paolino, MIT".
- Icons: packaging/macos/icon-1024.png and spindle.icns, packaging/windows/
  spindle.ico are Spindle's disc icon.
- `App::next_pass`: progress repaint 1000 ms when the window is unfocused
  (upstream 250 ms always).
- `theme::spinner`: 15 fps in focus, frozen out of focus (upstream 30 fps
  whenever visible). Measured on the Mac: a paused app with the library
  spinner showing sat at 17 to 20 percent CPU and 5 percent GPU; after the
  change 1.9 percent CPU with the spinner on screen. The top bar spinner
  stays up for minutes whenever Spotify throttles the shared connection, so
  this was the whole idle cost.
- Toast countdown repaint 250 ms when unfocused (upstream 33 ms).
- .cargo/config.toml links MediaPlayer for aarch64-apple-darwin.
- packaging/macos/bundle-linux.sh bundles without sips/iconutil/codesign;
  sign with `rcodesign sign`.

## Building here (Linux container)

Toolchain 1.98.0 (rust-toolchain.toml). Linux host build fails on libspa
(PipeWire headers), which does not matter: targets are macOS and Windows.
Always `--no-default-features` (drops MilkDrop, a C++ CMake build).

macOS: `. /home/claude/macsdk/env.sh && cargo +1.98.0 build --release
--no-default-features --target aarch64-apple-darwin`. env.sh sets the SDK,
the clang+lld linker wrapper and a CC wrapper for objc2's C helper.
Bundle: `packaging/macos/bundle-linux.sh <bin> "<out>/Spindle X.Y.Z.app" X.Y.Z`
then `rcodesign sign`, tar, device_commit_files to ~/Downloads/SpindleApp,
untar with device_bash (avoids quarantine). Launch via the bundle id.

Windows: `cargo +1.98.0 zigbuild --release --no-default-features --target
x86_64-pc-windows-gnu`, then the installer in /home/claude/spindle/installer
with SPINDLE_EXE pointing at the exe (autostart flag is `--start-hidden`).

## Facts from the live test (Mac, 2026-10-20)

- Sign in, playback enable and personal app authorization each open the
  browser once. Personal app redirect is http://127.0.0.1:8989/login (the
  old from-scratch Spindle used 8898).
- Playlists load in full through the session (732 songs), lyrics sync,
  friends feed works, EQ is under Settings > Equalizer.
- While the window is covered by other windows, screenshots are stale;
  take a display screenshot (which uncovers the app) before judging state.

## The old from-scratch app

/home/claude/spindle (0.1.9) is kept for reference: its backend/dsp.rs EQ,
lyrics.rs and the installer crate. Nothing in it is used by this tree except
the installer.
