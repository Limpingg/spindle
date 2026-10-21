//! Embeds spindle.exe (from SPINDLE_EXE) into the setup program, and the
//! icon and version information into the exe when it is built on another
//! system with Zig as the resource compiler, like the app's own build.rs.

use std::path::PathBuf;
use std::process::Command;

fn main() {
    println!("cargo:rerun-if-env-changed=SPINDLE_EXE");
    let out = PathBuf::from(std::env::var_os("OUT_DIR").expect("OUT_DIR"));
    let exe = std::env::var("SPINDLE_EXE").unwrap_or_default();
    // The exe is embedded deflated (about a third of its size); the setup
    // program inflates it while installing.
    let payload = out.join("spindle.exe.deflate");
    if exe.is_empty() {
        // `cargo check` without a payload still builds; the installer then
        // refuses to run, with a clear message.
        std::fs::write(&payload, []).expect("write empty payload");
        println!("cargo:warning=SPINDLE_EXE is not set: the setup program embeds nothing");
    } else {
        println!("cargo:rerun-if-changed={exe}");
        let bytes = std::fs::read(&exe).expect("read spindle.exe");
        let file = std::fs::File::create(&payload).expect("create payload");
        let mut encoder = flate2::write::DeflateEncoder::new(file, flate2::Compression::best());
        std::io::Write::write_all(&mut encoder, &bytes).expect("deflate spindle.exe");
        encoder.finish().expect("finish deflate");
    }

    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() != Ok("windows") {
        return;
    }
    let icon = std::fs::canonicalize("../packaging/windows/spindle.ico")
        .expect("packaging/windows/spindle.ico")
        .display()
        .to_string()
        .replace('\\', "/");
    println!("cargo:rerun-if-changed={icon}");
    let version = env!("CARGO_PKG_VERSION");
    let mut numbers: Vec<&str> = version.split(['.', '-']).take(3).collect();
    while numbers.len() < 3 {
        numbers.push("0");
    }
    let numeric = format!("{},{},{},0", numbers[0], numbers[1], numbers[2]);
    let script = format!(
        r#"1 ICON "{icon}"
1 VERSIONINFO
FILEVERSION {numeric}
PRODUCTVERSION {numeric}
BEGIN
  BLOCK "StringFileInfo"
  BEGIN
    BLOCK "040904b0"
    BEGIN
      VALUE "FileDescription", "Spindle Setup"
      VALUE "FileVersion", "{version}"
      VALUE "InternalName", "spindle-setup"
      VALUE "OriginalFilename", "spindle-setup.exe"
      VALUE "ProductName", "Spindle"
      VALUE "ProductVersion", "{version}"
    END
  END
  BLOCK "VarFileInfo"
  BEGIN
    VALUE "Translation", 0x409, 0x4b0
  END
END
"#
    );
    let rc = out.join("setup.rc");
    let res = out.join("setup.res");
    std::fs::write(&rc, script).expect("write resource script");
    let candidates: Vec<Vec<String>> = match std::env::var("SPINDLE_RC") {
        Ok(custom) => vec![custom.split_whitespace().map(String::from).collect()],
        Err(_) => vec![
            vec!["zig".into(), "rc".into()],
            vec!["python3".into(), "-m".into(), "ziglang".into(), "rc".into()],
        ],
    };
    for candidate in candidates {
        let Some((program, arguments)) = candidate.split_first() else {
            continue;
        };
        let status = Command::new(program)
            .args(arguments)
            .arg(&rc)
            .arg(&res)
            .status();
        if matches!(status, Ok(status) if status.success()) && res.is_file() {
            println!("cargo:rustc-link-arg-bins={}", res.display());
            return;
        }
    }
    println!("cargo:warning=Windows resources not embedded: no resource compiler (install Zig or set SPINDLE_RC)");
}
