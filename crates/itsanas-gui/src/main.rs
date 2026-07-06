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

const DEFAULT_BASE_URL: &str = "http://127.0.0.1:4279";
const STATUS_POLL_INTERVAL: Duration = Duration::from_secs(1);
const FILES_POLL_INTERVAL: Duration = Duration::from_secs(3);

#[derive(Deserialize, Default, Clone, PartialEq, Debug)]
struct Status {
    has_account: bool,
    unlocked: bool,
    synced_folder: String,
    #[serde(default)]
    vault_health: Option<VaultHealth>,
}

#[derive(Deserialize, Default, Clone, PartialEq, Debug)]
struct VaultHealth {
    healthy_shards: u64,
    unhealthy_files: Vec<String>,
}

#[derive(Deserialize, Clone, PartialEq, Debug)]
struct FileInfo {
    name: String,
    size: u64,
}

struct App {
    // Kept alive only to keep a daemon we spawned ourselves running for as
    // long as this app is open; never read otherwise.
    _daemon: Option<Child>,
    base_url: String,
    status: Option<Status>,
    last_status_poll: Instant,
    files: Vec<FileInfo>,
    last_files_poll: Instant,
    password: String,
    confirm_password: String,
    error: Option<String>,
}

impl App {
    fn new(base_url: String) -> Self {
        let daemon = ensure_daemon_running(&base_url);
        let far_past = Instant::now() - Duration::from_secs(3600);
        Self {
            _daemon: daemon,
            base_url,
            status: None,
            last_status_poll: far_past,
            files: Vec::new(),
            last_files_poll: far_past,
            password: String::new(),
            confirm_password: String::new(),
            error: None,
        }
    }

    /// Test-only constructor: skips `ensure_daemon_running` (the test
    /// harness starts its own in-process daemon and already knows it's
    /// up), so tests aren't racing a spawned child process.
    #[cfg(test)]
    fn new_for_test(base_url: String) -> Self {
        let far_past = Instant::now() - Duration::from_secs(3600);
        Self {
            _daemon: None,
            base_url,
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
        self.status = ureq::get(&format!("{}/status", self.base_url))
            .call()
            .ok()
            .and_then(|resp| resp.into_json::<Status>().ok());
    }

    fn refresh_files(&mut self) {
        self.last_files_poll = Instant::now();
        if let Some(files) = ureq::get(&format!("{}/files", self.base_url))
            .call()
            .ok()
            .and_then(|resp| resp.into_json::<Vec<FileInfo>>().ok())
        {
            self.files = files;
        }
    }

    /// Validates and submits the account-setup form. Pure state
    /// transition plus one HTTP call — no `egui` involved, so this is
    /// exactly the part unit tests exercise directly.
    fn do_setup(&mut self) {
        if self.password.is_empty() {
            self.error = Some("Password can't be empty.".to_string());
        } else if self.password != self.confirm_password {
            self.error = Some("Passwords don't match.".to_string());
        } else {
            match ureq::post(&format!("{}/account/setup", self.base_url))
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

    fn do_unlock(&mut self) {
        match ureq::post(&format!("{}/account/unlock", self.base_url))
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

    fn do_lock(&mut self) {
        let _ = ureq::post(&format!("{}/account/lock", self.base_url)).call();
        self.refresh_status();
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
            self.do_setup();
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
            self.do_unlock();
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
                self.do_lock();
            }
        });
        ui.separator();

        if let Some(health) = &status.vault_health {
            if !health.unhealthy_files.is_empty() {
                ui.colored_label(
                    egui::Color32::from_rgb(200, 60, 60),
                    format!(
                        "{} file(s) need attention (failed a background integrity check): {}",
                        health.unhealthy_files.len(),
                        health.unhealthy_files.join(", ")
                    ),
                );
                ui.add_space(8.0);
            }
        }

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
fn ensure_daemon_running(base_url: &str) -> Option<Child> {
    let already_running = ureq::get(&format!("{base_url}/status"))
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
    eframe::run_native(
        "itsanas",
        options,
        Box::new(|_cc| Ok(Box::new(App::new(DEFAULT_BASE_URL.to_string())))),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;

    /// Boots a real `itsanas-daemon` (the actual router + vault, not a
    /// mock) on an OS-assigned loopback port, backed by a fresh temp
    /// directory. Returns once it's actually answering requests.
    fn spawn_test_daemon() -> (String, tempfile::TempDir) {
        let dir = tempfile::tempdir().expect("tempdir");
        let state = Arc::new(
            itsanas_daemon::AppState::open(dir.path().join("data"), dir.path().join("synced"))
                .expect("open daemon state"),
        );
        let router = itsanas_daemon::http::router(state);

        let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("bind");
        listener.set_nonblocking(true).expect("set_nonblocking");
        let addr = listener.local_addr().expect("local_addr");
        let base_url = format!("http://{addr}");

        std::thread::spawn(move || {
            let rt = tokio::runtime::Runtime::new().expect("tokio runtime");
            rt.block_on(async move {
                let listener = tokio::net::TcpListener::from_std(listener).expect("from_std");
                axum::serve(listener, router)
                    .await
                    .expect("daemon server error");
            });
        });

        let deadline = Instant::now() + Duration::from_secs(5);
        while Instant::now() < deadline {
            if ureq::get(&format!("{base_url}/status")).call().is_ok() {
                return (base_url, dir);
            }
            std::thread::sleep(Duration::from_millis(20));
        }
        panic!("test daemon never came up");
    }

    #[test]
    fn refresh_status_is_none_when_nothing_is_listening() {
        // Port 1 is a reserved low port nothing will ever be bound to in
        // a test environment; connecting to it fails immediately.
        let mut app = App::new_for_test("http://127.0.0.1:1".to_string());
        app.refresh_status();
        assert_eq!(app.status, None);
    }

    #[test]
    fn fresh_daemon_has_no_account_and_is_locked() {
        let (base_url, _dir) = spawn_test_daemon();
        let mut app = App::new_for_test(base_url);

        app.refresh_status();

        let status = app.status.expect("status should be Some for a live daemon");
        assert!(!status.has_account);
        assert!(!status.unlocked);
    }

    #[test]
    fn setup_with_mismatched_passwords_fails_locally_without_reaching_an_unreachable_daemon() {
        // Base URL nothing is listening on: if do_setup() tried to reach
        // the network here, it would surface a "Setup failed" transport
        // error instead of the validation message — proving the mismatch
        // check runs before any HTTP call.
        let mut app = App::new_for_test("http://127.0.0.1:1".to_string());
        app.password = "one".to_string();
        app.confirm_password = "two".to_string();

        app.do_setup();

        assert_eq!(app.error.as_deref(), Some("Passwords don't match."));
    }

    #[test]
    fn setup_with_empty_password_fails_locally() {
        let mut app = App::new_for_test("http://127.0.0.1:1".to_string());
        app.password = "".to_string();
        app.confirm_password = "".to_string();

        app.do_setup();

        assert_eq!(app.error.as_deref(), Some("Password can't be empty."));
    }

    #[test]
    fn setup_then_status_reflects_an_unlocked_account() {
        let (base_url, _dir) = spawn_test_daemon();
        let mut app = App::new_for_test(base_url);
        app.password = "correct horse battery staple".to_string();
        app.confirm_password = "correct horse battery staple".to_string();

        app.do_setup();

        assert_eq!(app.error, None);
        assert!(
            app.password.is_empty(),
            "password field should be cleared after submit"
        );
        let status = app.status.expect("do_setup refreshes status");
        assert!(status.has_account);
        assert!(status.unlocked, "setup should leave the vault unlocked");
        assert_eq!(
            status.vault_health, None,
            "no background scrub has completed yet right after setup"
        );
    }

    #[test]
    fn unlock_with_wrong_password_reports_an_error_and_stays_locked() {
        let (base_url, _dir) = spawn_test_daemon();
        let mut app = App::new_for_test(base_url.clone());
        app.password = "the-real-password".to_string();
        app.confirm_password = "the-real-password".to_string();
        app.do_setup();
        app.do_lock();

        app.password = "a-wrong-guess".to_string();
        app.do_unlock();

        assert_eq!(app.error.as_deref(), Some("Wrong password."));
        app.refresh_status();
        assert!(!app.status.unwrap().unlocked);
    }

    #[test]
    fn unlock_with_correct_password_succeeds() {
        let (base_url, _dir) = spawn_test_daemon();
        let mut app = App::new_for_test(base_url);
        app.password = "the-real-password".to_string();
        app.confirm_password = "the-real-password".to_string();
        app.do_setup();
        app.do_lock();

        app.password = "the-real-password".to_string();
        app.do_unlock();

        assert_eq!(app.error, None);
        assert!(app.password.is_empty());
        let status = app.status.expect("do_unlock refreshes status");
        assert!(status.unlocked);
    }

    #[test]
    fn lock_then_status_reflects_locked() {
        let (base_url, _dir) = spawn_test_daemon();
        let mut app = App::new_for_test(base_url);
        app.password = "pw".to_string();
        app.confirm_password = "pw".to_string();
        app.do_setup();

        app.do_lock();

        let status = app.status.expect("do_lock refreshes status");
        assert!(!status.unlocked);
    }

    #[test]
    fn refresh_files_reflects_what_is_actually_in_the_vault() {
        let (base_url, _dir) = spawn_test_daemon();
        let mut app = App::new_for_test(base_url.clone());
        app.password = "pw".to_string();
        app.confirm_password = "pw".to_string();
        app.do_setup();

        ureq::put(&format!("{base_url}/files/notes.txt"))
            .send_bytes(b"hello from a test")
            .expect("put file");

        app.refresh_files();

        assert_eq!(app.files.len(), 1);
        assert_eq!(app.files[0].name, "notes.txt");
        assert_eq!(app.files[0].size, "hello from a test".len() as u64);
    }
}
