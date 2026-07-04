//! Desktop companion app for itsanas.
//!
//! It does *not* implement file transfer itself — that's the whole point
//! of the folder-sync engine in `itsanas-daemon`, which mirrors a real
//! local folder with the vault so the OS's normal file manager (drag and
//! drop, copy/paste, open) just works. This app is the small always-running
//! piece around that: create/unlock the account, show where the synced
//! folder lives, and offer a way in if the daemon isn't already running.

use std::path::PathBuf;
use std::process::{Child, Command};
use std::time::{Duration, Instant};

use eframe::egui;
use serde::Deserialize;

const BASE_URL: &str = "http://127.0.0.1:4279";
const STATUS_POLL_INTERVAL: Duration = Duration::from_secs(1);
const FILES_POLL_INTERVAL: Duration = Duration::from_secs(3);

#[derive(Deserialize, Default, Clone)]
struct Status {
    has_account: bool,
    unlocked: bool,
    synced_folder: String,
}

#[derive(Deserialize, Clone)]
struct FileInfo {
    name: String,
    size: u64,
}

struct App {
    // Kept alive only to keep a daemon we spawned ourselves running for as
    // long as this app is open; never read otherwise.
    _daemon: Option<Child>,
    status: Option<Status>,
    last_status_poll: Instant,
    files: Vec<FileInfo>,
    last_files_poll: Instant,
    password: String,
    confirm_password: String,
    error: Option<String>,
}

impl App {
    fn new() -> Self {
        let daemon = ensure_daemon_running();
        let far_past = Instant::now() - Duration::from_secs(3600);
        Self {
            _daemon: daemon,
            status: None,
            last_status_poll: far_past,
            files: Vec::new(),
            last_files_poll: far_past,
            password: String::new(),
            confirm_password: String::new(),
            error: None,
        }
    }

    fn refresh_status(&mut self) {
        self.last_status_poll = Instant::now();
        self.status = ureq::get(&format!("{BASE_URL}/status"))
            .call()
            .ok()
            .and_then(|resp| resp.into_json::<Status>().ok());
    }

    fn refresh_files(&mut self) {
        self.last_files_poll = Instant::now();
        if let Some(files) = ureq::get(&format!("{BASE_URL}/files"))
            .call()
            .ok()
            .and_then(|resp| resp.into_json::<Vec<FileInfo>>().ok())
        {
            self.files = files;
        }
    }

    fn setup_screen(&mut self, ui: &mut egui::Ui) {
        ui.label(
            "Create your account password. This encrypts everything in \
             your vault — there is no recovery if you lose it.",
        );
        ui.add_space(8.0);
        ui.add(
            egui::TextEdit::singleline(&mut self.password)
                .password(true)
                .hint_text("Password"),
        );
        ui.add(
            egui::TextEdit::singleline(&mut self.confirm_password)
                .password(true)
                .hint_text("Confirm password"),
        );
        ui.add_space(8.0);
        if ui.button("Create account").clicked() {
            if self.password.is_empty() {
                self.error = Some("Password can't be empty.".to_string());
            } else if self.password != self.confirm_password {
                self.error = Some("Passwords don't match.".to_string());
            } else {
                match ureq::post(&format!("{BASE_URL}/account/setup"))
                    .send_json(ureq::json!({ "password": self.password }))
                {
                    Ok(_) => {
                        self.error = None;
                        self.password.clear();
                        self.confirm_password.clear();
                        self.refresh_status();
                    }
                    Err(e) => self.error = Some(format!("Setup failed: {e}")),
                }
            }
        }
    }

    fn unlock_screen(&mut self, ui: &mut egui::Ui) {
        ui.label("Unlock your vault.");
        ui.add_space(8.0);
        ui.add(
            egui::TextEdit::singleline(&mut self.password)
                .password(true)
                .hint_text("Password"),
        );
        ui.add_space(8.0);
        if ui.button("Unlock").clicked() {
            match ureq::post(&format!("{BASE_URL}/account/unlock"))
                .send_json(ureq::json!({ "password": self.password }))
            {
                Ok(_) => {
                    self.error = None;
                    self.password.clear();
                    self.refresh_status();
                }
                Err(_) => self.error = Some("Wrong password.".to_string()),
            }
        }
    }

    fn main_screen(&mut self, ui: &mut egui::Ui, status: &Status) {
        ui.label("Unlocked.");
        ui.add_space(4.0);
        ui.horizontal(|ui| {
            ui.label("Synced folder:");
            ui.monospace(&status.synced_folder);
        });
        ui.add_space(4.0);
        ui.horizontal(|ui| {
            if ui.button("Open synced folder").clicked() {
                open_folder(&status.synced_folder);
            }
            if ui.button("Lock").clicked() {
                let _ = ureq::post(&format!("{BASE_URL}/account/lock")).call();
                self.refresh_status();
            }
        });
        ui.separator();

        if self.last_files_poll.elapsed() >= FILES_POLL_INTERVAL {
            self.refresh_files();
        }
        ui.label(format!("{} file(s) in the vault:", self.files.len()));
        egui::ScrollArea::vertical().show(ui, |ui| {
            for f in &self.files {
                ui.label(format!("{}  ({} bytes)", f.name, f.size));
            }
        });
    }
}

impl eframe::App for App {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        if self.last_status_poll.elapsed() >= STATUS_POLL_INTERVAL {
            self.refresh_status();
        }
        let status = self.status.clone();

        egui::CentralPanel::default().show(ctx, |ui| {
            ui.heading("itsanas");
            ui.add_space(8.0);

            match status {
                None => {
                    ui.label("Waiting for the itsanas-daemon service to start...");
                }
                Some(s) if !s.has_account => self.setup_screen(ui),
                Some(s) if !s.unlocked => self.unlock_screen(ui),
                Some(s) => self.main_screen(ui, &s),
            }

            if let Some(err) = self.error.clone() {
                ui.add_space(8.0);
                ui.colored_label(egui::Color32::from_rgb(200, 60, 60), err);
            }
        });

        ctx.request_repaint_after(Duration::from_millis(300));
    }
}

/// If the daemon isn't already listening, spawns it from next to this
/// binary (or falls back to `PATH`) so the GUI is a true double-click
/// entry point rather than requiring the daemon to be started separately.
fn ensure_daemon_running() -> Option<Child> {
    let already_running = ureq::get(&format!("{BASE_URL}/status"))
        .timeout(Duration::from_millis(300))
        .call()
        .is_ok();
    if already_running {
        return None;
    }

    let bin_name = if cfg!(windows) {
        "itsanas-daemon.exe"
    } else {
        "itsanas-daemon"
    };
    let alongside: Option<PathBuf> = std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(|dir| dir.join(bin_name)));

    let mut cmd = match alongside {
        Some(path) if path.exists() => Command::new(path),
        _ => Command::new(bin_name),
    };
    cmd.spawn().ok()
}

fn open_folder(path: &str) {
    #[cfg(target_os = "windows")]
    {
        let _ = Command::new("explorer").arg(path).spawn();
    }
    #[cfg(target_os = "macos")]
    {
        let _ = Command::new("open").arg(path).spawn();
    }
    #[cfg(all(unix, not(target_os = "macos")))]
    {
        let _ = Command::new("xdg-open").arg(path).spawn();
    }
}

fn main() -> eframe::Result<()> {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default().with_inner_size([420.0, 480.0]),
        ..Default::default()
    };
    eframe::run_native("itsanas", options, Box::new(|_cc| Ok(Box::new(App::new()))))
}
