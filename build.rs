//! Compiles translations, embeds Windows resources and links GLEW for libprojectM.

/// Known names for vcpkg's static GLEW library, in preferred order.
/// Names differ across vcpkg versions and triplets, so use the installed one.
#[cfg(windows)]
const GLEW_NAMES: &[&str] = &["glew32s", "libglew32", "glew32"];

/// Returns the vcpkg triplet matching the target architecture and CRT mode.
#[cfg(windows)]
fn vcpkg_triplet() -> Option<String> {
    let arch = match std::env::var("CARGO_CFG_TARGET_ARCH").as_deref() {
        Ok("x86_64") => "x64",
        Ok("aarch64") => "arm64",
        _ => return None,
    };
    let static_crt = std::env::var("CARGO_CFG_TARGET_FEATURE")
        .unwrap_or_default()
        .split(',')
        .any(|feature| feature == "crt-static");
    let suffix = if static_crt { "static" } else { "static-md" };
    Some(format!("{arch}-windows-{suffix}"))
}

/// Returns the known GLEW library installed in `lib`.
#[cfg(windows)]
fn glew_library(lib: &std::path::Path) -> Option<&'static str> {
    let present: Vec<String> = std::fs::read_dir(lib)
        .ok()?
        .flatten()
        .filter_map(|entry| {
            let path = entry.path();
            if path.extension()?.eq_ignore_ascii_case("lib") {
                Some(path.file_stem()?.to_str()?.to_ascii_lowercase())
            } else {
                None
            }
        })
        .collect();
    GLEW_NAMES
        .iter()
        .copied()
        .find(|name| present.iter().any(|found| found == name))
}

/// Embeds the icon and version information when a Windows exe is built on
/// another system, where winresource has no resource compiler. Zig ships
/// one (`zig rc`), and cargo-zigbuild already needs Zig, so the resource
/// script is compiled with it and linked into every binary. Without it the
/// exe builds, but Windows shows the generic window icon for it.
#[cfg(not(windows))]
fn embed_windows_resources_with_zig() {
    use std::path::PathBuf;
    use std::process::Command;

    let icon = PathBuf::from("packaging/windows/spindle.ico");
    println!("cargo:rerun-if-changed={}", icon.display());
    println!("cargo:rerun-if-env-changed=SPINDLE_RC");
    let out = PathBuf::from(std::env::var_os("OUT_DIR").expect("OUT_DIR"));
    let version = env!("CARGO_PKG_VERSION");
    let mut numbers: Vec<&str> = version.split(['.', '-']).take(3).collect();
    while numbers.len() < 3 {
        numbers.push("0");
    }
    let numeric = format!("{},{},{},0", numbers[0], numbers[1], numbers[2]);
    let icon_path = std::fs::canonicalize(&icon)
        .expect("icon")
        .display()
        .to_string()
        .replace('\\', "/");
    let script = format!(
        r#"1 ICON "{icon_path}"
1 VERSIONINFO
FILEVERSION {numeric}
PRODUCTVERSION {numeric}
BEGIN
  BLOCK "StringFileInfo"
  BEGIN
    BLOCK "040904b0"
    BEGIN
      VALUE "FileDescription", "Spindle"
      VALUE "FileVersion", "{version}"
      VALUE "InternalName", "spindle"
      VALUE "LegalCopyright", "Spindle is a fork of Spotifast by Carmine Paolino, MIT"
      VALUE "OriginalFilename", "spindle.exe"
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
    let rc = out.join("spindle.rc");
    let res = out.join("spindle.res");
    std::fs::write(&rc, script).expect("write resource script");

    // SPINDLE_RC names the compiler ("zig rc" or "python3 -m ziglang rc");
    // otherwise try both in turn.
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
    println!(
        "cargo:warning=Windows resources not embedded: no resource compiler (install Zig or set SPINDLE_RC)"
    );
}

fn main() {
    fastframe_i18n::build::compile_catalogs("assets/i18n");
    #[cfg(not(windows))]
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") {
        embed_windows_resources_with_zig();
    }
    #[cfg(windows)]
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") {
        println!("cargo:rerun-if-changed=packaging/windows/spindle.ico");
        let mut resource = winresource::WindowsResource::new();
        resource
            .set_icon("packaging/windows/spindle.ico")
            .set("ProductName", "Spindle")
            // Task Manager and the Windows mixer name the process by its description.
            .set("FileDescription", "Spindle");
        if let Err(error) = resource.compile() {
            println!("cargo:warning=Windows resources not embedded: {error}");
        }
        // Static libprojectM requires the GLEW library installed by vcpkg.
        if std::env::var_os("CARGO_FEATURE_MILKDROP").is_some() {
            println!("cargo:rerun-if-env-changed=VCPKG_INSTALLATION_ROOT");
            if let (Some(root), Some(triplet)) =
                (std::env::var_os("VCPKG_INSTALLATION_ROOT"), vcpkg_triplet())
            {
                let lib = std::path::Path::new(&root)
                    .join("installed")
                    .join(triplet)
                    .join("lib");
                println!("cargo:rustc-link-search=native={}", lib.display());
                match glew_library(&lib) {
                    Some(name) => println!("cargo:rustc-link-lib=static={name}"),
                    None => {
                        // Include the directory listing in the error for diagnosis.
                        let listing = std::fs::read_dir(&lib)
                            .map(|entries| {
                                entries
                                    .flatten()
                                    .map(|entry| entry.file_name().to_string_lossy().into_owned())
                                    .collect::<Vec<_>>()
                                    .join(", ")
                            })
                            .unwrap_or_else(|error| format!("unreadable: {error}"));
                        println!(
                            "cargo:warning=no GLEW library in {}; it holds: {listing}",
                            lib.display()
                        );
                    }
                }
            }
        }
    }
}
