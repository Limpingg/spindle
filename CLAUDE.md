# Spindle (fork of Spotifast)

This tree is crmne/spotifast (MIT), renamed to Spindle for Nick. AGENTS.md
is spotifast's own agent guide and still applies to the code; this file
adds what is different here.

Writing rule from Nick: no em dashes or en dashes anywhere, ever.

Never run `cargo fmt` over the whole tree: it reformats Spotifast's lines
and every reformatted line conflicts at the next merge. Format only files
Spindle owns (`rustfmt src/session_search.rs`), or none.

## How Spotifast releases flow in (rename, then merge)

History: Spotifast's commits, then "Rename Spotifast to Spindle" (the
mechanical rename by scripts/rename-upstream.py, branch spotifast-renamed),
then Nick's changes on main. Each Spotifast release is renamed the same way
and committed on spotifast-renamed with the release as a second parent,
then merged into main. Both sides carry identical renames, so merges
conflict only where Spindle's own changes meet Spotifast's.

- `scripts/sync-spotifast.sh [vX.Y.Z]` does one release. Exit 2 means
  conflicts are left in the tree to resolve and commit (or Cargo.lock on
  main differs from Spotifast's beyond the version line, which the script
  refuses to resolve blindly). Spotifast's tags are fetched under
  refs/tags/spotifast/ so they never mix or collide with Spindle's own v*
  tags (Spindle's v0.15.1 versus Spotifast's v0.15.1).
- `.github/workflows/spindle-sync-spotifast.yml` runs it daily, pushes a
  clean merge, and dispatches the Release workflow; conflicts become an
  issue titled "Spotifast merge needs a hand" for a person to run the
  script locally.
- `.github/workflows/spindle-release.yml` builds the Windows setup program
  and portable zip (cross compiled with Zig, like here) and the macOS dmg,
  writes checksums.txt and publishes the GitHub release the app's updater
  reads. It runs on a `v*` tag or by dispatch.
- Versions: Spindle's is Spotifast's major.minor with patch * 100 plus
  Spindle's own revision (Spotifast 0.14.1 is Spindle 0.14.101; a Spindle
  only change on top is 0.14.102). Always above the Spotifast release it
  carries and below the next, so the updater orders them. The tag must
  equal Cargo.toml's version.
- Never change the rename rules casually: a changed rule makes the next
  renamed base differ from the one main was built on. If a rule must
  change, apply it to main in the same commit.
- Spotifast's own workflows are dropped by the rename script (they would
  run and fail here); Spindle's are the `spindle-*.yml` files.

## Self updates

`updates.rs` points at Limpingg/spindle with no publisher key (the
releases are not signed yet; sign checksums.txt in the Release workflow and
put the public key in assets/update-public-key.hex to require it).
Settings default `check_for_updates: true`. Never point the updater back at
crmne/spotifast: with the PC's "download automatically" on, the app offered
"Update to 0.14.1", which would have installed Spotifast over Spindle.
The setup program accepts Inno Setup's silent flags (/VERYSILENT /DIR=
/LOG=), which is how fastframe-update runs it, and writes
spindle-installer.txt beside the exe so the updater knows the copy was
installed that way. The Mac build is Apple silicon only
(`MacTarget::Arm64Only`, asset spindle-v<version>-macos-arm64.dmg).

## The icon

Nick's own picture, packaging/icons/source.png: neon green headphones on a
head with a sound wave, on a dark green rounded square (it replaced the
mirror ball on 2026-10-21). packaging/icons/make-icons.py cuts the square
out and writes every icon from it: spindle-1024.png (rounded square, clear
corners: window icon, Windows tray, `theme::logo` through
`util::app_icon_rgba`), spindle-mac-1024.png and packaging/macos/icon-1024.png
(Apple's grid), spindle-tray-1024.png (the neon lines alone, the macOS menu
bar template through `util::tray_template_rgba`), spindle.svg (the Linux
tray, a PNG inside an SVG without filters), packaging/windows/spindle.ico,
packaging/macos/spindle.icns and installer/assets/window-icon.png. Rerun it
when the picture changes. No Spotify mark on any icon: it is Spotify's
trademark.

## What changed from upstream

- The rename is scripts/rename-upstream.py (see above): Spotifast ->
  Spindle, spotifast -> spindle, bundle id com.nickaustin.spindle, bin
  src/spindle.rs, assets/skins/spindle.wsz, contrib/omarchy/spindle*.
  Upstream links (spotifast.rocks, github.com/crmne/spotifast), the credit
  and the translation template's name stay.
- Home: a Made for you or New releases shelf whose only content is a rate
  limit or Development Mode quota failure is hidden (`ui/home.rs`
  `shelf_state`) instead of showing an error row with Retry; the next Home
  refresh reads it again. The PC log showed 150 shared-app rate limit hits
  in 8 minutes with the Retry buttons in play.
- Spotifast 0.14.1 merged (the sink queues 200 ms of sound measured in
  time, not twelve packets). The PC log had nine WASAPI underruns and one
  "audio queue ran dry" on the GoXLR before it.
- Windows resources when cross compiling: build.rs (app and installer)
  writes an .rc with the icon and version info, compiles it with `zig rc`
  (or `python3 -m ziglang rc`, or whatever SPINDLE_RC names) and links the
  .res with `cargo:rustc-link-arg-bins`. winresource only runs on a Windows
  host, which is why the first PC build had no taskbar icon.
- About shows "Spindle is a fork of Spotifast by Carmine Paolino, MIT".
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

## Speed work (0.14.103)

- `timeline.rs` logs at INFO: `startup: <milestone> at N ms` (first frame,
  signed in, playback session ready, playlists listed, home loaded, friends
  loaded), `play: sound after N ms (<uri>)` per play, and the backend logs
  `slow: <ApiRequest kind> answered after N ms` for any read over 700 ms.
  Read Nick's spindle.log for these before guessing at what is slow.
- Sign in: the first /me check runs `api::client::impatient`, so a throttled
  shared app answers at once and the remembered account
  (verified_user_<source>.json in the data folder) signs in instantly; the
  PC log showed the old path waiting 32 s inside the client's own retries
  before the remembered account was even consulted. The remembered file is
  keyed on the client id only (PKCE rotates the refresh token on every
  refresh, so a key with it matched only after short runs) and deleted on
  sign out and when a grant is forgotten.
- Reads the personal app serves wait up to 5 s for its sign-in verdict
  (`ApiGateway::client_for`) instead of going to the throttled shared app
  when the shared app signed in first from the remembered account.
- The setup program takes its version from the root Cargo.toml (build.rs
  sets SPINDLE_VERSION); installer/Cargo.toml's own version is 0.0.0 and
  never bumped. Uninstall deletes the updater's .spindle-update-* staging
  folders and removes itself through a batch file in the temp folder.
- Friends: polled every 15 s with the panel open and every 45 s with it
  closed while the session is up, refreshed the moment the session comes
  up or back. A read the session's end cut short now answers with a
  failure; before, `friends_in_flight` stayed set forever and the panel
  never refreshed again after a reconnect.
- Session reads fall back to the Web API after 10 s, not 30.

## Search, the Keychain and releases (Mac, 2026-10-21)

- Search goes through Pathfinder's `searchDesktop` (src/session_search.rs,
  hash from the web player build of 2026-10-21). Spotify answered it for a
  playback session started from the browser approval and refused it (403
  "Client/request not allowed") in every request form for the same
  session restored from its saved login. So the approval's refresh token
  is kept in `search-grant.json` in the state folder, and search asks with
  an access token from it first, then with Login5. Without either, the Web
  API search answers (personal app for the playlist half while the shared
  app is throttled).
- Each build is signed anew, so after an update macOS shows a Keychain
  prompt before handing over the saved grants. Credential reads wait five
  minutes for it (credentials.rs TIMEOUT); at 20 seconds two of the three
  grants timed out and playback and the personal app had to be approved
  again.
- Two releases dispatched close together publish in either order, and
  GitHub marks the last published as Latest: check that the newest version
  is Latest (edit the release, Set as the latest) or the updater offers
  the older one.
- Releases from 0.14.107 and older know the repository as
  Nicolasaustin/spindle and reject assets GitHub now serves from
  Limpingg/spindle ("Update asset does not belong to this release"); such a
  copy needs one manual install.

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
