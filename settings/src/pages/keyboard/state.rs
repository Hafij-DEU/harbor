//! What the Keyboard Manager cards remember between frames (`App::keyboard`),
//! the once-a-second refresh of keyboards and service state, and saving.

use std::sync::mpsc::{self, Receiver};
use std::time::{Duration, Instant};

use eframe::egui::Ui;

use crate::app::App;
use crate::keyboard::config::Config;
use crate::keyboard::devices::{self, Keyboard, Target};
use crate::keyboard::status::Status;
use crate::system;

/// How often keyboards and service state are read again.
const REFRESH: Duration = Duration::from_secs(1);

/// A helper call running on a thread.
pub struct Task {
    /// The config being saved: it becomes `saved` when the call succeeds.
    pub config: Option<Config>,
    /// Toast shown on success.
    pub done: &'static str,
    pub rx: Receiver<Result<(), String>>,
}

#[derive(Default)]
pub struct KeyboardUi {
    /// What /etc/harbor/keyboard.conf holds (None until the page is shown).
    pub saved: Option<Config>,
    /// Remap edits not applied yet; only `remap` and `rules` are used.
    pub draft: Option<Config>,
    pub task: Option<Task>,
    /// A Type button waiting for a key: (rule index, 0 = from / 1 = to).
    pub capture: Option<(usize, u8)>,
    /// Remap rules shown as editors; the rest show as a list row with
    /// Edit and Remove. New rules open here; Apply closes them all.
    pub open: Vec<usize>,
    pub key_check: super::key_check::KeyCheckUi,
    pub boards: Vec<Keyboard>,
    pub status: Option<Status>,
    pub service_running: bool,
    pub toggle_watchdog: bool,
    /// Stuck keys card: Advanced (keys that count, times) shown.
    pub guard_advanced: bool,
    refreshed: Option<Instant>,
}

impl KeyboardUi {
    pub fn busy(&self) -> bool {
        self.task.is_some()
    }

    /// Keys typed now belong to this page (a Type button or Key check),
    /// so the app's own keyboard shortcuts pause.
    pub fn wants_keys(&self) -> bool {
        self.capture.is_some() || self.key_check.active()
    }

    /// The saved config (loaded on first use).
    pub fn saved(&mut self) -> &mut Config {
        self.saved.get_or_insert_with(Config::load)
    }

    /// Name to show for a keyboard target.
    pub fn board_name(&self, target: &Target) -> String {
        match target {
            Target::Every => "Every keyboard".to_owned(),
            Target::Internal => "Laptop keyboard".to_owned(),
            Target::Id(id) => self
                .boards
                .iter()
                .find(|k| &k.id == id)
                .map_or_else(|| format!("Keyboard {id} (unplugged)"), |k| k.name.clone()),
        }
    }
}

/// Per frame: finish a running helper call and refresh state once a second.
pub fn refresh(app: &mut App, ui: &mut Ui) {
    if let Some(task) = &app.keyboard.task {
        match task.rx.try_recv() {
            Ok(result) => {
                let task = app.keyboard.task.take().expect("checked above");
                match result {
                    Ok(()) => {
                        if let Some(cfg) = task.config {
                            app.keyboard.saved = Some(cfg);
                        }
                        app.keyboard.refreshed = None; // show the new state now
                        app.notify(task.done);
                    }
                    Err(err) => app.notify_error(&err),
                }
            }
            Err(mpsc::TryRecvError::Empty) => {
                ui.ctx().request_repaint_after(Duration::from_millis(200))
            }
            Err(mpsc::TryRecvError::Disconnected) => app.keyboard.task = None,
        }
    }
    let k = &mut app.keyboard;
    if k.saved.is_none() {
        k.saved = Some(Config::load());
    }
    if k.draft.is_none() {
        k.draft = k.saved.clone();
    }
    if k.refreshed.is_none_or(|t| t.elapsed() >= REFRESH) {
        k.refreshed = Some(Instant::now());
        k.boards = devices::keyboards();
        k.status = Status::read();
        k.service_running = system::keyboard_service_running();
        k.toggle_watchdog = system::toggle_watchdog_running();
    }
    ui.ctx().request_repaint_after(REFRESH);
}

/// Save `config` through the helper on a thread.
pub fn save(app: &mut App, config: Config, done: &'static str) {
    let text = config.text();
    run(app, Some(config), done, move || {
        system::save_keyboard_config(&text)
    });
}

/// Run any helper call on a thread.
pub fn run(
    app: &mut App,
    config: Option<Config>,
    done: &'static str,
    call: impl FnOnce() -> Result<(), String> + Send + 'static,
) {
    if app.keyboard.busy() {
        return;
    }
    let (tx, rx) = mpsc::channel();
    std::thread::spawn(move || {
        let _ = tx.send(call());
    });
    app.keyboard.task = Some(Task { config, done, rx });
}
