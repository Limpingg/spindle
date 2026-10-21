//! Spindle's Windows setup program: a small egui window over Nick's picture
//! that installs spindle.exe for the current user, with no administrator
//! prompt. It also serves as the uninstaller: the copy written next to the
//! app as uninstall.exe runs with `--uninstall`.
//!
//! What it does:
//! - copies spindle.exe, uninstall.exe, README.md and LICENSE to
//!   %LOCALAPPDATA%\Programs\spindle (closing a running Spindle first),
//! - makes a Start menu shortcut and, if wanted, a desktop shortcut,
//! - registers in Settings > Apps > Installed apps so Windows can remove it
//!   the normal way,
//! - and, with `--uninstall`, undoes all of the above.
//!
//! Starting with Windows is Spindle's own setting, so the setup program
//! leaves the Run key alone.

#![cfg_attr(windows, windows_subsystem = "windows")]

use std::path::{Path, PathBuf};
use std::sync::mpsc::{Receiver, channel};
use std::thread;

use eframe::egui::{self, Color32, CornerRadius, Margin, RichText, Vec2};

const APP_NAME: &str = "Spindle";
/// The app's version, read by build.rs from the repository's Cargo.toml, so
/// the setup program, Apps & features and spindle.exe always agree.
const VERSION: &str = env!("SPINDLE_VERSION");
/// spindle.exe, deflated by build.rs.
const PAYLOAD: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/spindle.exe.deflate"));
/// Windows hides console windows of child processes started with this flag.
#[cfg(windows)]
const CREATE_NO_WINDOW: u32 = 0x0800_0000;

/// The exe, inflated.
fn payload() -> Result<Vec<u8>, String> {
    use std::io::Read;
    if PAYLOAD.is_empty() {
        return Err("This setup program was built without spindle.exe inside it.".into());
    }
    let mut bytes = Vec::with_capacity(PAYLOAD.len() * 3);
    flate2::read::DeflateDecoder::new(PAYLOAD)
        .read_to_end(&mut bytes)
        .map_err(|error| format!("The embedded spindle.exe is damaged: {error}"))?;
    if !bytes.starts_with(b"MZ") {
        return Err("The embedded spindle.exe is not a Windows program.".into());
    }
    Ok(bytes)
}

/// A child process that opens no console window.
fn quiet(program: &str) -> std::process::Command {
    #[allow(unused_mut)]
    let mut command = std::process::Command::new(program);
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        command.creation_flags(CREATE_NO_WINDOW);
    }
    command
}
const README: &str = include_str!("../../README.md");
const LICENSE: &str = include_str!("../../LICENSE");
const BACKGROUND: &[u8] = include_bytes!("../assets/background.png");
const WINDOW_ICON: &[u8] = include_bytes!("../assets/window-icon.png");
const WINDOW_SIZE: Vec2 = Vec2::new(760.0, 420.0);
/// Settings > Apps lists the program under this key, which never changes.
const UNINSTALL_KEY: &str = r"Software\Microsoft\Windows\CurrentVersion\Uninstall\Spindle";

/// Spindle's own updater runs the setup program the way it runs Inno
/// Setup: silently, into the folder the running copy lives in, with a log.
/// Only these flags matter; the others Inno accepts are ignored.
struct Silent {
    directory: Option<PathBuf>,
    log: Option<PathBuf>,
}

fn silent_flags() -> Option<Silent> {
    let arguments: Vec<String> = std::env::args().skip(1).collect();
    let silent = arguments
        .iter()
        .any(|argument| matches!(argument.to_ascii_uppercase().as_str(), "/VERYSILENT" | "/SILENT" | "--SILENT"));
    if !silent {
        return None;
    }
    let value = |prefix: &str| {
        arguments
            .iter()
            .find_map(|argument| {
                let upper = argument.to_ascii_uppercase();
                upper
                    .starts_with(prefix)
                    .then(|| argument[prefix.len()..].trim_matches('"').to_owned())
            })
            .filter(|value| !value.is_empty())
            .map(PathBuf::from)
    };
    Some(Silent {
        directory: value("/DIR="),
        log: value("/LOG="),
    })
}

/// Whether this process is the uninstall.exe copy: run from Apps &
/// features it gets `--uninstall`, but a double click gives no arguments.
fn running_as_uninstaller() -> bool {
    std::env::current_exe().is_ok_and(|own| {
        own.file_name()
            .is_some_and(|name| name.eq_ignore_ascii_case("uninstall.exe"))
    })
}

fn main() -> eframe::Result<()> {
    if let Some(silent) = silent_flags() {
        let lines = std::cell::RefCell::new(Vec::new());
        let result = install_into(silent.directory.clone(), false, true, &|message: &str| {
            lines.borrow_mut().push(message.to_owned());
        });
        if let Some(log) = &silent.log {
            let mut text = lines.borrow().join("\n");
            if let Err(error) = &result {
                text.push_str("\nerror: ");
                text.push_str(error);
            }
            text.push('\n');
            let _ = std::fs::write(log, text);
        }
        std::process::exit(if result.is_ok() { 0 } else { 1 });
    }
    let uninstall =
        std::env::args().any(|argument| argument == "--uninstall") || running_as_uninstaller();
    let icon = image::load_from_memory(WINDOW_ICON)
        .map(|image| {
            let rgba = image.to_rgba8();
            egui::IconData {
                width: rgba.width(),
                height: rgba.height(),
                rgba: rgba.into_raw(),
            }
        })
        .ok();
    let mut viewport = egui::ViewportBuilder::default()
        .with_inner_size(WINDOW_SIZE)
        .with_min_inner_size(WINDOW_SIZE)
        .with_max_inner_size(WINDOW_SIZE)
        .with_resizable(false)
        .with_maximize_button(false)
        .with_title(if uninstall {
            "Remove Spindle"
        } else {
            "Spindle Setup"
        });
    if let Some(icon) = icon {
        viewport = viewport.with_icon(icon);
    }
    let options = eframe::NativeOptions {
        viewport,
        centered: true,
        ..Default::default()
    };
    eframe::run_native(
        "Spindle Setup",
        options,
        Box::new(move |cc| Ok(Box::new(Setup::new(cc, uninstall)))),
    )
}

/// Where everything goes for this user.
struct Places {
    install_dir: PathBuf,
    exe: PathBuf,
    uninstaller: PathBuf,
}

impl Places {
    /// The folder this uninstaller lives in, else the per user default.
    fn discover() -> Result<Self, String> {
        if running_as_uninstaller()
            && let Ok(own) = std::env::current_exe()
            && let Some(directory) = own.parent()
        {
            return Ok(Self::in_directory(directory.to_path_buf()));
        }
        let local = std::env::var_os("LOCALAPPDATA")
            .map(PathBuf::from)
            .filter(|path| path.is_dir())
            .ok_or("LOCALAPPDATA is not set")?;
        Ok(Self::in_directory(local.join("Programs").join("spindle")))
    }

    fn in_directory(install_dir: PathBuf) -> Self {
        Self {
            exe: install_dir.join("spindle.exe"),
            uninstaller: install_dir.join("uninstall.exe"),
            install_dir,
        }
    }
}

enum Stage {
    Welcome,
    Working(Receiver<Result<String, String>>, Vec<String>),
    Done(Result<(), String>),
}

struct Setup {
    uninstall: bool,
    desktop_shortcut: bool,
    launch: bool,
    stage: Stage,
    background: Option<egui::TextureHandle>,
}

impl Setup {
    fn new(cc: &eframe::CreationContext<'_>, uninstall: bool) -> Self {
        let background = image::load_from_memory(BACKGROUND).ok().map(|image| {
            let rgba = image.to_rgba8();
            let size = [rgba.width() as usize, rgba.height() as usize];
            cc.egui_ctx.load_texture(
                "background",
                egui::ColorImage::from_rgba_unmultiplied(size, rgba.as_raw()),
                egui::TextureOptions::LINEAR,
            )
        });
        cc.egui_ctx.set_theme(egui::Theme::Dark);
        cc.egui_ctx.all_styles_mut(|style| {
            style.spacing.button_padding = Vec2::new(18.0, 8.0);
        });
        Self {
            uninstall,
            desktop_shortcut: true,
            launch: true,
            stage: Stage::Welcome,
            background,
        }
    }

    fn start(&mut self) {
        let (sender, receiver) = channel();
        let uninstall = self.uninstall;
        let desktop_shortcut = self.desktop_shortcut;
        thread::spawn(move || {
            let report = |message: &str| {
                let _ = sender.send(Ok(message.to_owned()));
            };
            let result = if uninstall {
                remove(&report)
            } else {
                install(desktop_shortcut, &report)
            };
            if let Err(error) = result {
                let _ = sender.send(Err(error));
            }
        });
        self.stage = Stage::Working(receiver, Vec::new());
    }
}

impl eframe::App for Setup {
    fn ui(&mut self, root: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let ctx = root.ctx().clone();
        let ctx = &ctx;
        // Collect progress from the worker.
        if let Stage::Working(receiver, log) = &mut self.stage {
            let mut finished = None;
            loop {
                match receiver.try_recv() {
                    Ok(Ok(message)) => log.push(message),
                    Ok(Err(error)) => {
                        finished = Some(Err(error));
                        break;
                    }
                    Err(std::sync::mpsc::TryRecvError::Empty) => {
                        ctx.request_repaint_after(std::time::Duration::from_millis(100));
                        break;
                    }
                    Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                        finished = Some(Ok(()));
                        break;
                    }
                }
            }
            if let Some(result) = finished {
                self.stage = Stage::Done(result);
            }
        }

        let frame = egui::Frame::NONE.fill(Color32::BLACK);
        egui::CentralPanel::default().frame(frame).show(root, |ui| {
            let rect = ui.max_rect();
            if let Some(texture) = &self.background {
                ui.painter().image(
                    texture.id(),
                    rect,
                    egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)),
                    Color32::WHITE,
                );
            }
            // The panel sits on the right, over the quiet part of the picture.
            let panel = egui::Rect::from_min_size(
                egui::pos2(rect.right() - 330.0, rect.top() + 24.0),
                Vec2::new(306.0, rect.height() - 48.0),
            );
            ui.painter().rect_filled(
                panel,
                CornerRadius::same(12),
                Color32::from_rgba_unmultiplied(12, 12, 14, 215),
            );
            let mut inner = ui.new_child(egui::UiBuilder::new().max_rect(panel.shrink2(Vec2::new(18.0, 16.0))));
            inner.vertical(|ui| self.panel(ui, ctx));
        });
    }
}

impl Setup {
    fn panel(&mut self, ui: &mut egui::Ui, ctx: &egui::Context) {
        let title = if self.uninstall {
            "Remove Spindle"
        } else {
            "Spindle"
        };
        ui.label(RichText::new(title).size(26.0).strong().color(Color32::WHITE));
        ui.label(
            RichText::new(format!("Version {VERSION}"))
                .size(13.0)
                .color(Color32::from_gray(170)),
        );
        ui.add_space(10.0);
        match &self.stage {
            Stage::Welcome => self.welcome(ui),
            Stage::Working(_, log) => {
                let log = log.clone();
                ui.label(RichText::new(if self.uninstall {
                    "Removing..."
                } else {
                    "Installing..."
                })
                .size(15.0)
                .color(Color32::WHITE));
                ui.add_space(6.0);
                for line in log.iter().rev().take(8).rev() {
                    ui.label(RichText::new(line).size(12.0).color(Color32::from_gray(200)));
                }
                ui.add_space(8.0);
                ui.add(egui::Spinner::new().size(22.0));
            }
            Stage::Done(result) => {
                let result = result.clone();
                self.done(ui, ctx, result);
            }
        }
    }

    fn welcome(&mut self, ui: &mut egui::Ui) {
        let places = Places::discover();
        if self.uninstall {
            ui.label(
                RichText::new("This removes Spindle and its shortcuts from this user account.")
                    .size(13.0)
                    .color(Color32::from_gray(220)),
            );
            ui.add_space(4.0);
            ui.label(
                RichText::new("Your settings and sign in stay in AppData, so a later install picks them back up.")
                    .size(12.0)
                    .color(Color32::from_gray(170)),
            );
        } else {
            ui.label(
                RichText::new("Spindle installs for your user account only, so Windows asks for no administrator password.")
                    .size(13.0)
                    .color(Color32::from_gray(220)),
            );
            ui.add_space(6.0);
            if let Ok(places) = &places {
                ui.label(
                    RichText::new(format!("Install folder:\n{}", places.install_dir.display()))
                        .size(11.0)
                        .color(Color32::from_gray(160)),
                );
            }
            ui.add_space(8.0);
            ui.checkbox(&mut self.desktop_shortcut, "Create a desktop shortcut");
            ui.checkbox(&mut self.launch, "Start Spindle when setup finishes");
            if PAYLOAD.is_empty() {
                ui.add_space(6.0);
                ui.colored_label(
                    Color32::from_rgb(255, 120, 120),
                    "This setup program was built without spindle.exe inside it.",
                );
            }
        }
        ui.add_space(14.0);
        let label = if self.uninstall { "Remove" } else { "Install" };
        let enabled = places.is_ok() && (self.uninstall || !PAYLOAD.is_empty());
        ui.horizontal(|ui| {
            if ui
                .add_enabled(enabled, egui::Button::new(RichText::new(label).size(15.0)))
                .clicked()
            {
                self.start();
            }
            if ui.button(RichText::new("Cancel").size(15.0)).clicked() {
                ui.ctx().send_viewport_cmd(egui::ViewportCommand::Close);
            }
        });
        if let Err(error) = places {
            ui.add_space(6.0);
            ui.colored_label(Color32::from_rgb(255, 120, 120), error);
        }
        ui.with_layout(egui::Layout::bottom_up(egui::Align::LEFT), |ui| {
            ui.label(
                RichText::new("Spindle is a fork of Spotifast by Carmine Paolino, MIT.")
                    .size(10.5)
                    .color(Color32::from_gray(130)),
            );
        });
    }

    fn done(&mut self, ui: &mut egui::Ui, ctx: &egui::Context, result: Result<(), String>) {
        match result {
            Ok(()) => {
                ui.label(
                    RichText::new(if self.uninstall {
                        "Spindle has been removed."
                    } else {
                        "Spindle is installed."
                    })
                    .size(16.0)
                    .color(Color32::WHITE),
                );
                if !self.uninstall {
                    ui.add_space(6.0);
                    ui.label(
                        RichText::new("You will find it in the Start menu. The first start opens your browser to sign in to Spotify.")
                            .size(12.0)
                            .color(Color32::from_gray(190)),
                    );
                    ui.add_space(8.0);
                    ui.checkbox(&mut self.launch, "Start Spindle now");
                }
                ui.add_space(14.0);
                if ui.button(RichText::new("Finish").size(15.0)).clicked() {
                    if self.uninstall {
                        delete_self_after_exit();
                    } else if self.launch
                        && let Ok(places) = Places::discover()
                    {
                        let _ = std::process::Command::new(&places.exe)
                            .current_dir(&places.install_dir)
                            .spawn();
                    }
                    ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                }
            }
            Err(error) => {
                ui.label(
                    RichText::new(if self.uninstall {
                        "Spindle could not be removed."
                    } else {
                        "Spindle could not be installed."
                    })
                    .size(16.0)
                    .color(Color32::from_rgb(255, 120, 120)),
                );
                ui.add_space(6.0);
                egui::Frame::NONE.inner_margin(Margin::same(4)).show(ui, |ui| {
                    ui.label(RichText::new(error).size(12.0).color(Color32::from_gray(210)));
                });
                ui.add_space(14.0);
                if ui.button(RichText::new("Close").size(15.0)).clicked() {
                    ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                }
            }
        }
    }
}

/// Installs everything, reporting each step.
fn install(desktop_shortcut: bool, report: &dyn Fn(&str)) -> Result<(), String> {
    install_into(None, desktop_shortcut, false, report)
}

/// Installs into `directory`, or the per user Programs folder. An update
/// keeps the shortcuts it finds and adds none on the desktop. A silent
/// update (`lenient`) succeeds once spindle.exe is in place: a shortcut or
/// registry step that fails is logged, not a reason for the updater to put
/// the old version back.
fn install_into(
    directory: Option<PathBuf>,
    desktop_shortcut: bool,
    lenient: bool,
    report: &dyn Fn(&str),
) -> Result<(), String> {
    let places = match directory {
        Some(directory) => Places::in_directory(directory),
        None => Places::discover()?,
    };
    report("Closing Spindle if it is running");
    close_running_app();
    report("Creating the install folder");
    std::fs::create_dir_all(&places.install_dir)
        .map_err(|error| format!("Couldn't create {}: {error}", places.install_dir.display()))?;
    report("Unpacking spindle.exe");
    let exe_bytes = payload()?;
    write_replacing(&places.exe, &exe_bytes)?;
    report("Copying the uninstaller");
    let own = std::env::current_exe().map_err(|error| format!("Couldn't find this program: {error}"))?;
    let own_bytes = std::fs::read(&own).map_err(|error| format!("Couldn't read {}: {error}", own.display()))?;
    if own.canonicalize().ok() != places.uninstaller.canonicalize().ok() {
        write_replacing(&places.uninstaller, &own_bytes)?;
    }
    write_replacing(&places.install_dir.join("README.md"), README.as_bytes())?;
    write_replacing(&places.install_dir.join("LICENSE"), LICENSE.as_bytes())?;
    // Tells Spindle's updater this copy was installed by the setup program,
    // so an update downloads the next setup program and runs it silently.
    write_replacing(
        &places.install_dir.join("spindle-installer.txt"),
        b"spindle-installer-v1\n",
    )?;
    report("Creating shortcuts");
    let tolerate = |step: &str, result: Result<(), String>| -> Result<(), String> {
        match result {
            Ok(()) => Ok(()),
            Err(error) if lenient => {
                report(&format!("{step} failed, continuing: {error}"));
                Ok(())
            }
            Err(error) => Err(error),
        }
    };
    tolerate("Shortcuts", shortcuts(&places, desktop_shortcut))?;
    report("Registering with Windows");
    tolerate("Registration", register(&places, exe_bytes.len() + own_bytes.len()))?;
    report("Done");
    Ok(())
}

/// Removes everything, reporting each step. The uninstaller itself is
/// deleted by a command that waits until this process has exited.
fn remove(report: &dyn Fn(&str)) -> Result<(), String> {
    let places = Places::discover()?;
    report("Closing Spindle if it is running");
    close_running_app();
    report("Removing shortcuts");
    remove_shortcuts();
    report("Unregistering from Windows");
    unregister();
    report("Deleting the program files");
    for name in ["spindle.exe", "README.md", "LICENSE", "spindle-installer.txt"] {
        let path = places.install_dir.join(name);
        if path.exists() {
            std::fs::remove_file(&path)
                .map_err(|error| format!("Couldn't delete {}: {error}", path.display()))?;
        }
    }
    // The updater's staging folders (.spindle-update-<hex>, holding the
    // previous exe and the downloaded setup program) and any .tmp left by
    // an interrupted write go too, or the folder could never be removed.
    if let Ok(entries) = std::fs::read_dir(&places.install_dir) {
        for entry in entries.flatten() {
            let name = entry.file_name().to_string_lossy().into_owned();
            let path = entry.path();
            if path.is_dir() && name.starts_with(".spindle-update-") {
                let _ = std::fs::remove_dir_all(&path);
            } else if path.is_file() && name.ends_with(".tmp") {
                let _ = std::fs::remove_file(&path);
            }
        }
    }
    // The running uninstaller cannot delete itself: `delete_self_after_exit`
    // does that once the window closes.
    report("Done");
    Ok(())
}

/// Starts a batch file that waits for this process to exit, deletes
/// uninstall.exe, removes the folder and deletes itself. It runs from the
/// temp folder, so it does not hold the install folder open, and a batch
/// file (not `cmd /c`) is what allows the retry loop. `ping` is the sleep:
/// `timeout` refuses to run without a console.
fn delete_self_after_exit() {
    let Ok(places) = Places::discover() else {
        return;
    };
    let uninstaller = places.uninstaller.display().to_string();
    let directory = places.install_dir.display().to_string();
    let script = format!(
        "@echo off\r\n\
         :wait\r\n\
         del /q \"{uninstaller}\" >nul 2>&1\r\n\
         if exist \"{uninstaller}\" (ping -n 2 127.0.0.1 >nul & goto wait)\r\n\
         rmdir \"{directory}\" >nul 2>&1\r\n\
         del \"%~f0\"\r\n"
    );
    let batch = std::env::temp_dir().join("spindle-remove.cmd");
    if std::fs::write(&batch, script).is_err() {
        return;
    }
    let _ = quiet("cmd")
        .arg("/c")
        .arg(&batch)
        .current_dir(std::env::temp_dir())
        .spawn();
}

/// Writes a file, replacing one that is there, through a temporary file
/// so a crash cannot leave half an exe behind.
fn write_replacing(path: &Path, bytes: &[u8]) -> Result<(), String> {
    let temporary = path.with_extension("tmp");
    std::fs::write(&temporary, bytes)
        .map_err(|error| format!("Couldn't write {}: {error}", temporary.display()))?;
    if path.exists() {
        std::fs::remove_file(path)
            .map_err(|error| format!("Couldn't replace {}: {error}", path.display()))?;
    }
    std::fs::rename(&temporary, path)
        .map_err(|error| format!("Couldn't move {} into place: {error}", path.display()))
}

/// Asks a running Spindle to close, then makes sure it is gone before the
/// exe is replaced. taskkill is part of every Windows install.
fn close_running_app() {
    let _ = quiet("taskkill")
        .args(["/IM", "spindle.exe", "/F"])
        .output();
    std::thread::sleep(std::time::Duration::from_millis(600));
}

/// Runs a PowerShell script without a console window.
fn powershell(script: &str) -> Result<(), String> {
    let output = quiet("powershell")
        .args([
            "-NoProfile",
            "-NonInteractive",
            "-ExecutionPolicy",
            "Bypass",
            "-WindowStyle",
            "Hidden",
            "-Command",
            script,
        ])
        .output()
        .map_err(|error| format!("Couldn't run PowerShell: {error}"))?;
    if output.status.success() {
        Ok(())
    } else {
        Err(format!(
            "PowerShell failed: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        ))
    }
}

/// Start menu shortcut, and a desktop one when wanted or already there (an
/// update rewrites the shortcuts it finds and adds none). The shell's own
/// Desktop folder is used, which follows a OneDrive redirected desktop.
fn shortcuts(places: &Places, desktop: bool) -> Result<(), String> {
    let exe = places.exe.display().to_string().replace('\'', "''");
    let dir = places.install_dir.display().to_string().replace('\'', "''");
    let want_desktop = if desktop { "$true" } else { "$false" };
    let mut script = format!(
        "$shell = New-Object -ComObject WScript.Shell\n\
         $targets = @([IO.Path]::Combine($shell.SpecialFolders.Item('Programs'), '{APP_NAME}.lnk'))\n\
         $desktop = [IO.Path]::Combine($shell.SpecialFolders.Item('Desktop'), '{APP_NAME}.lnk')\n\
         if ({want_desktop} -or (Test-Path $desktop)) {{ $targets += $desktop }}\n"
    );
    script.push_str(&format!(
        "foreach ($target in $targets) {{\n\
           $link = $shell.CreateShortcut($target)\n\
           $link.TargetPath = '{exe}'\n\
           $link.WorkingDirectory = '{dir}'\n\
           $link.IconLocation = '{exe},0'\n\
           $link.Description = 'Spindle'\n\
           $link.Save()\n\
         }}\n"
    ));
    powershell(&script)
}

fn remove_shortcuts() {
    let script = format!(
        "$shell = New-Object -ComObject WScript.Shell\n\
         foreach ($folder in @('Programs', 'Desktop')) {{\n\
           $target = [IO.Path]::Combine($shell.SpecialFolders.Item($folder), '{APP_NAME}.lnk')\n\
           if (Test-Path $target) {{ Remove-Item -Force $target }}\n\
         }}\n"
    );
    let _ = powershell(&script);
}

#[cfg(windows)]
fn register(places: &Places, size_bytes: usize) -> Result<(), String> {
    use winreg::RegKey;
    use winreg::enums::HKEY_CURRENT_USER;
    let user = RegKey::predef(HKEY_CURRENT_USER);
    let (key, _) = user
        .create_subkey(UNINSTALL_KEY)
        .map_err(|error| format!("Couldn't register the program: {error}"))?;
    let exe = places.exe.display().to_string();
    let uninstaller = places.uninstaller.display().to_string();
    let set = |name: &str, value: &str| {
        key.set_value(name, &value)
            .map_err(|error| format!("Couldn't write {name}: {error}"))
    };
    set("DisplayName", APP_NAME)?;
    set("DisplayVersion", VERSION)?;
    set("Publisher", "Nick Austin")?;
    set("DisplayIcon", &exe)?;
    set("InstallLocation", &places.install_dir.display().to_string())?;
    set("UninstallString", &format!("\"{uninstaller}\" --uninstall"))?;
    set("QuietUninstallString", &format!("\"{uninstaller}\" --uninstall"))?;
    set("URLInfoAbout", "https://github.com/Nicolasaustin/spindle")?;
    key.set_value("NoModify", &1u32)
        .map_err(|error| format!("Couldn't write NoModify: {error}"))?;
    key.set_value("NoRepair", &1u32)
        .map_err(|error| format!("Couldn't write NoRepair: {error}"))?;
    key.set_value("EstimatedSize", &((size_bytes / 1024) as u32))
        .map_err(|error| format!("Couldn't write EstimatedSize: {error}"))?;
    Ok(())
}

#[cfg(not(windows))]
fn register(_places: &Places, _size_bytes: usize) -> Result<(), String> {
    Ok(())
}

#[cfg(windows)]
fn unregister() {
    use winreg::RegKey;
    use winreg::enums::HKEY_CURRENT_USER;
    let _ = RegKey::predef(HKEY_CURRENT_USER).delete_subkey_all(UNINSTALL_KEY);
}

#[cfg(not(windows))]
fn unregister() {}
