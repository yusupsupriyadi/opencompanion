//! Updates from the GitHub releases, through the Tauri updater plugin. The flow is Orca's
//! (`src/main/updater/`): a check after start and once a day, a download only once the owner
//! presses Update, then a restart. Changed: Orca shows an in-app card only; this also sends one OS
//! notification per version, so a window hidden in the tray hears about it. Orca keeps its
//! terminals through the restart and this app cannot, so the window warns before it stops sessions.

use std::sync::{Arc, Mutex, MutexGuard};
use std::time::{Duration, Instant};

use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager};
use tauri_plugin_updater::{Update, UpdaterExt};

use crate::db::{now_ms, Db, UpdateMarks};

/// The first check waits until the window and the sessions are up.
const FIRST_CHECK: Duration = Duration::from_secs(30);
/// The background loop wakes this often and checks once a day has passed on the wall clock:
/// tokio's clock stops while the computer sleeps, so a 24-hour sleep can last days.
const TICK: Duration = Duration::from_secs(3600);
const CHECK_EVERY_MS: i64 = 24 * 3600 * 1000;
/// Download progress reaches the window at most this often.
const PROGRESS_EVERY: Duration = Duration::from_millis(200);

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum Phase {
    #[default]
    Idle,
    Checking,
    Downloading,
    Installing,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum Step {
    Check,
    Install,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Failure {
    pub step: Step,
    /// The updater's own English message, shown under the window's translated sentence.
    pub message: String,
}

/// What Settings › Updates and the sidebar card show.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateView {
    /// The running version.
    pub current: String,
    /// A newer release the last check found.
    pub available: Option<String>,
    /// The sidebar card of `available` was closed with Later.
    pub dismissed: bool,
    pub phase: Phase,
    /// Bytes downloaded so far, and the size when the server sends it.
    pub received: u64,
    pub total: Option<u64>,
    /// When the last check that reached GitHub finished, in ms.
    pub checked_at: Option<i64>,
    /// A check the owner asked for, or an install, that failed. A failed check in the background
    /// stays quiet and runs again on the next tick.
    pub error: Option<Failure>,
}

struct Inner {
    view: UpdateView,
    found: Option<Update>,
}

pub struct Updates {
    db: Arc<Db>,
    inner: Mutex<Inner>,
    /// Stops the sessions before the Windows installer starts: the plugin then ends the process
    /// with `std::process::exit`, which skips `RunEvent::Exit`.
    before_exit: Arc<dyn Fn() + Send + Sync>,
}

impl Updates {
    fn lock(&self) -> MutexGuard<'_, Inner> {
        self.inner.lock().unwrap_or_else(|e| e.into_inner())
    }

    fn view(&self) -> UpdateView {
        self.lock().view.clone()
    }

    fn changed(&self, app: &AppHandle) {
        let _ = app.emit("update-changed", self.view());
    }
}

/// Registers the state and starts the background checks.
pub fn manage(app: &AppHandle, db: Arc<Db>, before_exit: impl Fn() + Send + Sync + 'static) {
    let view = UpdateView { current: app.package_info().version.to_string(), ..UpdateView::default() };
    app.manage(Updates {
        db,
        inner: Mutex::new(Inner { view, found: None }),
        before_exit: Arc::new(before_exit),
    });
    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        tokio::time::sleep(FIRST_CHECK).await;
        loop {
            let updates = app.state::<Updates>();
            let on = updates.db.settings().map(|s| s.update_check).unwrap_or(true);
            if on && due(updates.view().checked_at, now_ms()) {
                check(&app, false).await;
            }
            tokio::time::sleep(TICK).await;
        }
    });
}

/// A check is due when none has reached GitHub yet, or the last one is a day old.
fn due(checked_at: Option<i64>, now: i64) -> bool {
    checked_at.is_none_or(|at| now - at >= CHECK_EVERY_MS)
}

/// Writes what a check found into `view`. Returns the version to tell the owner about, once.
fn settle(view: &mut UpdateView, found: Option<&str>, marks: &UpdateMarks, now: i64) -> Option<String> {
    view.checked_at = Some(now);
    view.error = None;
    view.available = found.map(str::to_string);
    view.dismissed = found.is_some_and(|v| v == marks.dismissed);
    found.filter(|v| *v != marks.notified).map(str::to_string)
}

/// Asks GitHub for a newer release. `asked` is a press of Check for updates: its failure is shown,
/// and it sends no OS notification, as the owner is already looking.
pub async fn check(app: &AppHandle, asked: bool) -> UpdateView {
    let updates = app.state::<Updates>();
    {
        let mut inner = updates.lock();
        if inner.view.phase != Phase::Idle {
            return inner.view.clone();
        }
        inner.view.phase = Phase::Checking;
        if asked {
            inner.view.error = None;
        }
    }
    updates.changed(app);
    let hook = Arc::clone(&updates.before_exit);
    let result = match app.updater_builder().on_before_exit(move || hook()).build() {
        Ok(updater) => updater.check().await,
        Err(e) => Err(e),
    };
    let mut marks = updates.db.update_marks().unwrap_or_default();
    let fresh = {
        let mut inner = updates.lock();
        inner.view.phase = Phase::Idle;
        match result {
            Ok(found) => {
                let fresh = settle(&mut inner.view, found.as_ref().map(|u| u.version.as_str()), &marks, now_ms());
                inner.found = found;
                fresh
            }
            Err(e) => {
                if asked {
                    inner.view.error = Some(Failure { step: Step::Check, message: e.to_string() });
                } else {
                    eprintln!("The update check failed: {e}");
                }
                None
            }
        }
    };
    updates.changed(app);
    if let Some(version) = fresh {
        if !asked {
            notify(app, &updates.db, &version);
        }
        marks.notified = version;
        let _ = updates.db.save_update_marks(&marks);
    }
    updates.view()
}

/// The OS notification, in the UI language. A click brings the window forward, where the sidebar
/// card has the Update button.
fn notify(app: &AppHandle, db: &Db, version: &str) {
    let current = app.package_info().version.to_string();
    let (title, body) = if db.settings().is_ok_and(|s| s.language == "id") {
        (format!("OpenCompanion {version} tersedia"), format!("Anda memakai {current}. Buka OpenCompanion untuk memperbarui."))
    } else {
        (format!("OpenCompanion {version} is available"), format!("You have {current}. Open OpenCompanion to update."))
    };
    crate::show_notification(app, &title, &body, "");
}

#[tauri::command]
pub fn update_status(app: AppHandle) -> UpdateView {
    app.state::<Updates>().view()
}

#[tauri::command]
pub async fn check_update(app: AppHandle) -> UpdateView {
    check(&app, true).await
}

/// Hides the sidebar card until a newer version comes out. Settings › Updates still offers it.
#[tauri::command]
pub fn dismiss_update(app: AppHandle) -> Result<UpdateView, String> {
    let updates = app.state::<Updates>();
    let Some(version) = updates.view().available else { return Ok(updates.view()) };
    let mut marks = updates.db.update_marks()?;
    marks.dismissed = version;
    updates.db.save_update_marks(&marks)?;
    updates.lock().view.dismissed = true;
    updates.changed(&app);
    Ok(updates.view())
}

/// Downloads the release the last check found, installs it and restarts. On Windows the installer
/// takes over and closes the app; on macOS and Linux the restart runs `RunEvent::Exit`, which stops
/// the sessions. Every running session ends either way.
#[tauri::command]
pub async fn install_update(app: AppHandle) -> Result<(), String> {
    let updates = app.state::<Updates>();
    let update = {
        let mut inner = updates.lock();
        if inner.view.phase != Phase::Idle {
            return Ok(());
        }
        let Some(update) = inner.found.clone() else {
            return Err("No update was found. Check for updates first.".into());
        };
        inner.view.phase = Phase::Downloading;
        inner.view.received = 0;
        inner.view.total = None;
        inner.view.error = None;
        update
    };
    updates.changed(&app);
    let mut sent = Instant::now();
    let result = update
        .download_and_install(
            |chunk, total| {
                {
                    let mut inner = updates.lock();
                    inner.view.received += chunk as u64;
                    inner.view.total = total;
                }
                if sent.elapsed() >= PROGRESS_EVERY {
                    sent = Instant::now();
                    updates.changed(&app);
                }
            },
            || {
                updates.lock().view.phase = Phase::Installing;
                updates.changed(&app);
            },
        )
        .await;
    match result {
        Ok(()) => {
            app.request_restart();
            Ok(())
        }
        Err(e) => {
            let message = e.to_string();
            {
                let mut inner = updates.lock();
                inner.view.phase = Phase::Idle;
                inner.view.error = Some(Failure { step: Step::Install, message: message.clone() });
            }
            updates.changed(&app);
            Err(message)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_check_is_due_at_first_and_after_a_day() {
        assert!(due(None, 0));
        assert!(!due(Some(1_000), 1_000 + CHECK_EVERY_MS - 1));
        assert!(due(Some(1_000), 1_000 + CHECK_EVERY_MS));
    }

    #[test]
    fn a_new_version_is_told_once() {
        let mut view = UpdateView::default();
        let none = UpdateMarks::default();
        assert_eq!(settle(&mut view, Some("0.3.0"), &none, 5), Some("0.3.0".into()));
        assert_eq!(view.available.as_deref(), Some("0.3.0"));
        assert_eq!(view.checked_at, Some(5));
        let told = UpdateMarks { notified: "0.3.0".into(), ..UpdateMarks::default() };
        assert_eq!(settle(&mut view, Some("0.3.0"), &told, 6), None);
        assert_eq!(settle(&mut view, Some("0.3.1"), &told, 7), Some("0.3.1".into()));
    }

    #[test]
    fn later_hides_only_the_version_it_was_pressed_for() {
        let mut view = UpdateView::default();
        let marks = UpdateMarks { dismissed: "0.3.0".into(), ..UpdateMarks::default() };
        settle(&mut view, Some("0.3.0"), &marks, 1);
        assert!(view.dismissed);
        settle(&mut view, Some("0.3.1"), &marks, 2);
        assert!(!view.dismissed);
    }

    #[test]
    fn being_up_to_date_clears_the_offer_and_an_old_error() {
        let mut view = UpdateView {
            available: Some("0.3.0".into()),
            error: Some(Failure { step: Step::Check, message: "offline".into() }),
            ..UpdateView::default()
        };
        assert_eq!(settle(&mut view, None, &UpdateMarks::default(), 3), None);
        assert_eq!(view.available, None);
        assert_eq!(view.error, None);
    }
}
