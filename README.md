# Spindle

Spindle is a fast, lightweight Spotify player for Windows and Mac, built by
Nick Austin to play music next to a game without stutters, hitches or
input lag. It stays light while a game has focus: it redraws less often
and keeps extra sound queued for the audio device.

- Your playlists, albums, artists and podcasts, with search
- Spotify Connect: play here or on any of your devices, and pick the
  sound output (GoXLR channels included)
- Start a Jam and share it with a link or a QR code
- Synced lyrics, the queue, and what your friends are listening to
- Equalizer
- Updates itself when a new version comes out

Spindle is not affiliated with Spotify.

## Download

| | |
|---|---|
| **Windows** | [Spindle-Setup.exe](https://github.com/Limpingg/spindle/releases/latest/download/Spindle-Setup.exe) |
| Windows, no install | [Spindle-Windows-Portable.zip](https://github.com/Limpingg/spindle/releases/latest/download/Spindle-Windows-Portable.zip) |
| **Mac** (Apple silicon: M1 and newer) | [Spindle-Mac.dmg](https://github.com/Limpingg/spindle/releases/latest/download/Spindle-Mac.dmg) |

You need Spotify Premium. The first time, Spindle walks you through
making a free app of your own on Spotify's developer dashboard (a few
minutes, with each step shown on the sign-in screen) and signs you in
through your browser. After that it signs in by itself and updates itself.

**Windows:** the setup program installs for your user only (no
administrator prompt), adds a Start menu shortcut and, if you leave the box
checked, a desktop shortcut, and registers in Settings > Apps so you can
remove it the normal way. Windows SmartScreen warns the first time because
the files are not code signed: choose "More info", then "Run anyway".
Start Spindle normally, not as administrator.

**Mac:** drag Spindle into Applications. macOS blocks the first open
because the app is not notarized: open System Settings > Privacy &
Security, scroll down and choose "Open Anyway". If macOS says Spindle is
damaged, run `xattr -dr com.apple.quarantine /Applications/Spindle.app` in
Terminal, then open it again.

Settings and your Spotify sign in are kept under
`%APPDATA%\paolino\spindle` and `%LOCALAPPDATA%\paolino\spindle` on
Windows, so reinstalling or updating keeps them.

## Build

    cargo build --release --no-default-features

MilkDrop is left out on purpose (a C++ visualizer is the opposite of what a
music player next to a game should run). From Linux or macOS, build the
Windows exe and installer with Zig as the linker and resource compiler:

    pip install ziglang && cargo install cargo-zigbuild
    rustup target add x86_64-pc-windows-gnu
    cargo zigbuild --release --no-default-features --target x86_64-pc-windows-gnu
    cd installer && SPINDLE_EXE=$PWD/../target/x86_64-pc-windows-gnu/release/spindle.exe \
      cargo zigbuild --release --target x86_64-pc-windows-gnu

The setup program lands in `installer/target/x86_64-pc-windows-gnu/release/spindle-setup.exe`.

## License

MIT. See [LICENSE](LICENSE).
