//! The desktop shell: a window, and a thin command layer over `starfleet-core`.

use starfleet_core::paths::Paths;
use starfleet_core::preflight::Check;
use starfleet_core::stations::Roster;
use starfleet_core::view::BoardView;
use starfleet_core::{DispatchRequest, DispatchResult, Options, Starfleet};
use std::sync::Arc;
use tauri::{Emitter, Manager, State};

/// The core, or the reason it could not start (shown by the UI instead of
/// the app dying on, say, a hand-edited roster with a typo).
struct Core(Result<Arc<Starfleet>, String>);

impl Core {
    fn get(&self) -> Result<&Arc<Starfleet>, String> {
        self.0.as_ref().map_err(Clone::clone)
    }
}

type CmdResult<T> = Result<T, String>;

/// Errors cross to the UI as one readable line. The alternate format makes
/// `anyhow` errors include their causes.
fn text(e: impl std::fmt::Display) -> String {
    format!("{e:#}")
}

#[tauri::command]
fn board_view(core: State<Core>) -> CmdResult<BoardView> {
    Ok(core.get()?.view())
}

#[tauri::command]
fn roster(core: State<Core>) -> CmdResult<Roster> {
    Ok(core.get()?.roster().clone())
}

#[tauri::command]
async fn preflight(core: State<'_, Core>) -> CmdResult<Vec<Check>> {
    Ok(core.get()?.preflight().await)
}

#[tauri::command]
async fn create_card(
    core: State<'_, Core>,
    title: String,
    cwd: String,
    notes: String,
) -> CmdResult<String> {
    core.get()?
        .create_card(&title, &cwd, &notes)
        .await
        .map_err(text)
}

#[tauri::command]
async fn update_card(
    core: State<'_, Core>,
    id: String,
    title: Option<String>,
    cwd: Option<String>,
    notes: Option<String>,
    done: Option<bool>,
) -> CmdResult<()> {
    core.get()?
        .update_card(&id, title, cwd, notes, done)
        .await
        .map_err(text)
}

#[tauri::command]
fn delete_card(core: State<Core>, id: String) -> CmdResult<()> {
    core.get()?.delete_card(&id).map_err(text)
}

#[tauri::command]
fn attach(core: State<Core>, card_id: String, pane_id: String) -> CmdResult<()> {
    core.get()?.attach(&card_id, &pane_id).map_err(text)
}

#[tauri::command]
fn detach(core: State<Core>, link_id: String) -> CmdResult<()> {
    core.get()?.detach(&link_id).map_err(text)
}

#[tauri::command]
async fn dispatch(core: State<'_, Core>, request: DispatchRequest) -> CmdResult<DispatchResult> {
    core.get()?.dispatch(request).await.map_err(text)
}

#[tauri::command]
async fn send_pending(core: State<'_, Core>, link_id: String) -> CmdResult<()> {
    core.get()?.send_pending(&link_id).await.map_err(text)
}

#[tauri::command]
async fn follow_up(core: State<'_, Core>, pane_id: String, text: String) -> CmdResult<()> {
    core.get()?
        .follow_up(&pane_id, &text)
        .await
        .map_err(self::text)
}

#[tauri::command]
async fn reply_keys(
    core: State<'_, Core>,
    pane_id: String,
    text: Option<String>,
    keys: Vec<String>,
) -> CmdResult<()> {
    core.get()?
        .reply_keys(&pane_id, text, keys)
        .await
        .map_err(self::text)
}

#[tauri::command]
async fn tail(core: State<'_, Core>, pane_id: String, lines: u32) -> CmdResult<String> {
    core.get()?.tail(&pane_id, lines).await.map_err(text)
}

#[tauri::command]
fn saved_output(core: State<Core>, link_id: String) -> CmdResult<Option<String>> {
    core.get()?.saved_output(&link_id).map_err(text)
}

#[tauri::command]
async fn focus(core: State<'_, Core>, pane_id: String) -> CmdResult<()> {
    core.get()?.focus(&pane_id).await.map_err(text)
}

fn start_core() -> Result<Arc<Starfleet>, String> {
    let paths = Paths::resolve().map_err(text)?;
    Starfleet::new(Options {
        paths,
        endpoint: None,
        env: None,
    })
    .map_err(text)
}

pub fn run() {
    tauri::Builder::default()
        .setup(|app| {
            let core = start_core();
            if let Ok(starfleet) = &core {
                tauri::async_runtime::spawn(starfleet.clone().run_monitor());
                let mut changes = starfleet.changes();
                let handle = app.handle().clone();
                tauri::async_runtime::spawn(async move {
                    while changes.changed().await.is_ok() {
                        let _ = handle.emit("board-changed", ());
                    }
                });
            }
            app.manage(Core(core));
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            board_view,
            roster,
            preflight,
            create_card,
            update_card,
            delete_card,
            attach,
            detach,
            dispatch,
            send_pending,
            follow_up,
            reply_keys,
            tail,
            saved_output,
            focus
        ])
        .run(tauri::generate_context!())
        .expect("failed to start the Starfleet window");
}
