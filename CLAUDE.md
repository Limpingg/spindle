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
- Settings default `check_for_updates: false`, and `updates.rs` points at
  Nicolasaustin/spindle with no publisher key. Never point it back at
  crmne/spotifast: Nick's PC settings had both update toggles on, and the
  app offered "Update to 0.14.1", which would have installed Spotifast over
  Spindle. There are no Spindle releases yet, so a check finds nothing. To
  turn self updates on, sign checksums.txt in a release workflow and put
  the public key in assets/update-public-key.hex.
- Home: a Made for you or New releases shelf whose only content is a rate
  limit or Development Mode quota failure is hidden (`ui/home.rs`
  `shelf_state`) instead of showing an error row with Retry; the next Home
  refresh reads it again. The PC log showed 150 shared-app rate limit hits
  in 8 minutes with the Retry buttons in play.
- Cherry picked upstream c85c038 (0.14.1): the sink queues 200 ms of sound
  measured in time, not twelve packets. The PC log had nine WASAPI underruns
  and one "audio queue ran dry" on the GoXLR before it.
- Windows resources when cross compiling: build.rs (app and installer)
  writes an .rc with the icon and version info, compiles it with `zig rc`
  (or `python3 -m ziglang rc`, or whatever SPINDLE_RC names) and links the
  .res with `cargo:rustc-link-arg-bins`. winresource only runs on a Windows
  host, which is why the first PC build had no taskbar icon.
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
x86_64-pc-windows-gnu`, then the installer:

    cd installer && SPINDLE_EXE=$PWD/../target/x86_64-pc-windows-gnu/release/spindle.exe \
      cargo +1.98.0 zigbuild --release --target x86_64-pc-windows-gnu

installer/ is its own small crate (egui, in this repository since the old
workspace that held it was lost): spindle.exe embedded through build.rs,
Nick's picture as the background, per user install to
%LOCALAPPDATA%\Programs\spindle, Start menu and optional desktop shortcut
through WScript.Shell in PowerShell, Apps & features registration through
winreg; the copied uninstall.exe is the same binary run with --uninstall.
Starting with Windows is the app's own setting (fastframe_autostart), so
the installer never touches the Run key. Deliver as
Spindle-<version>-Setup.exe into Nick's Downloads with device_commit_files.
Needs the PipeWire and ALSA headers only to run the test suite on Linux:
`apt-get install libpipewire-0.3-dev libasound2-dev`.

## Facts from the live test (Mac, 2026-10-20)

- Sign in, playback enable and personal app authorization each open the
  browser once. Personal app redirect is http://127.0.0.1:8989/login (the
  old from-scratch Spindle used 8898).
- Playlists load in full through the session (732 songs), lyrics sync,
  friends feed works, EQ is under Settings > Equalizer.
- While the window is covered by other windows, screenshots are stale;
  take a display screenshot (which uncovers the app) before judging state.

## Facts from the live test (Windows PC, 2026-10-20)

- Spindle 0.2.0 playing, window visible, no game: spindle.exe 0.1 percent
  CPU, 150 MB, RTX 4090, GoXLR Music channel through WASAPI.
- Settings live in %APPDATA%\paolino\spindle\config\settings.json, the
  log in %LOCALAPPDATA%\paolino\spindle\data\spindle.log.
- Task Manager runs elevated, so computer use cannot click in it; read the
  Processes list and ask Nick for the GPU column.

## The old from-scratch app

The 0.1.x from-scratch Spindle lived in an earlier workspace that is gone.
Nothing from it is used here.
