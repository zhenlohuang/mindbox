use std::{io, time::Duration};

use anyhow::Result;
use crossterm::{
    execute,
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
};
use ratatui::{Terminal, backend::CrosstermBackend};
use tokio::sync::mpsc;

use crate::client::MindboxClient;

mod app;
mod event;
mod render;

use app::App;
use event::{
    AppEvent, spawn_log_dir_watcher, spawn_resource_poller, spawn_sse_reader, spawn_task_poller,
    spawn_terminal_events, spawn_tick,
};

pub async fn run(client: &MindboxClient, task_id: &str) -> Result<()> {
    let mut guard = TerminalGuard::activate()?;
    let backend = CrosstermBackend::new(io::stdout());
    let mut terminal = Terminal::new(backend)?;
    terminal.clear()?;

    let (tx, mut rx) = mpsc::channel::<AppEvent>(256);
    let client_for_sse = client.clone();
    let client_for_poller = client.clone();
    let client_for_resources = client.clone();
    let task_id_owned = task_id.to_string();

    spawn_terminal_events(tx.clone());
    spawn_sse_reader(client_for_sse, task_id_owned.clone(), tx.clone());
    spawn_task_poller(client_for_poller, task_id_owned.clone(), tx.clone());
    spawn_resource_poller(client_for_resources, tx.clone());
    spawn_log_dir_watcher(task_id_owned.clone(), tx.clone());
    spawn_tick(tx, Duration::from_millis(200));

    let mut app = App::new(task_id_owned);
    terminal.draw(|frame| render::draw(frame, &mut app))?;

    while let Some(event) = rx.recv().await {
        app.handle(event);
        terminal.draw(|frame| render::draw(frame, &mut app))?;
        if app.should_quit {
            break;
        }
    }

    guard.deactivate();
    terminal.show_cursor()?;
    Ok(())
}

struct TerminalGuard {
    active: bool,
}

impl TerminalGuard {
    fn activate() -> Result<Self> {
        enable_raw_mode()?;
        execute!(io::stdout(), EnterAlternateScreen)?;
        Ok(Self { active: true })
    }

    fn deactivate(&mut self) {
        if !self.active {
            return;
        }

        let _ = disable_raw_mode();
        let _ = execute!(io::stdout(), LeaveAlternateScreen);
        self.active = false;
    }
}

impl Drop for TerminalGuard {
    fn drop(&mut self) {
        self.deactivate();
    }
}
