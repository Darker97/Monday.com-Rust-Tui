mod api;
mod app;
mod config;
mod ui;

use anyhow::{Context, Result};
use app::{App, Msg, Popup, Screen};
use crossterm::{
    event::{
        DisableMouseCapture, EnableMouseCapture, Event, EventStream, KeyCode, KeyEvent,
        KeyEventKind, KeyModifiers,
    },
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use futures::StreamExt;
use ratatui::{backend::CrosstermBackend, Terminal};
use std::io::{self, Stdout};
use std::time::Duration;
use tokio::sync::mpsc;

type Term = Terminal<CrosstermBackend<Stdout>>;

#[tokio::main]
async fn main() -> Result<()> {
    let cfg = config::Config::load().unwrap_or_default();
    let has_token = cfg.has_token();
    let token = if has_token {
        cfg.auth.api_token.clone()
    } else {
        String::new()
    };

    let client = api::MondayClient::new(token).context("init client")?;
    let (tx, mut rx) = mpsc::unbounded_channel::<Msg>();
    let mut app = App::new(client, tx, has_token);
    if has_token {
        app.load_workspaces();
    }

    let mut terminal = setup_terminal()?;
    let res = run_app(&mut terminal, &mut app, &mut rx).await;
    restore_terminal(&mut terminal)?;
    res
}

fn setup_terminal() -> Result<Term> {
    enable_raw_mode().context("enable raw mode")?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    Ok(Terminal::new(backend)?)
}

fn restore_terminal(terminal: &mut Term) -> Result<()> {
    disable_raw_mode().ok();
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        DisableMouseCapture
    )
    .ok();
    terminal.show_cursor().ok();
    Ok(())
}

async fn run_app(
    terminal: &mut Term,
    app: &mut App,
    rx: &mut mpsc::UnboundedReceiver<Msg>,
) -> Result<()> {
    let mut events = EventStream::new();
    let mut tick = tokio::time::interval(Duration::from_millis(200));

    loop {
        terminal.draw(|f| ui::render(f, app))?;
        if app.should_quit {
            return Ok(());
        }

        tokio::select! {
            maybe_event = events.next() => {
                if let Some(Ok(ev)) = maybe_event {
                    handle_event(app, ev)?;
                }
            }
            Some(msg) = rx.recv() => {
                app.handle_msg(msg);
            }
            _ = tick.tick() => {
                app.clear_old_toasts();
            }
        }
    }
}

fn handle_event(app: &mut App, ev: Event) -> Result<()> {
    let Event::Key(key) = ev else {
        return Ok(());
    };
    if key.kind != KeyEventKind::Press {
        return Ok(());
    }

    if key.modifiers.contains(KeyModifiers::CONTROL) && key.code == KeyCode::Char('c') {
        app.should_quit = true;
        return Ok(());
    }

    if app.screen == Screen::TokenPrompt {
        handle_token_prompt(app, key);
        return Ok(());
    }

    if app.popup != Popup::None {
        handle_popup(app, key);
        return Ok(());
    }

    handle_screen(app, key);
    Ok(())
}

fn handle_token_prompt(app: &mut App, key: KeyEvent) {
    match key.code {
        KeyCode::Esc => app.should_quit = true,
        KeyCode::Enter => {
            let token = app.token_input.trim().to_string();
            if token.is_empty() {
                app.set_error("Token cannot be empty.");
                return;
            }
            let cfg = config::Config {
                auth: config::AuthConfig {
                    api_token: token.clone(),
                },
            };
            if let Err(e) = cfg.save() {
                app.set_error(format!("Save token: {}", e));
                return;
            }
            match api::MondayClient::new(token) {
                Ok(client) => {
                    app.client = std::sync::Arc::new(client);
                    app.screen = Screen::Workspaces;
                    app.token_input.clear();
                    app.load_workspaces();
                }
                Err(e) => app.set_error(format!("Client: {}", e)),
            }
        }
        KeyCode::Backspace => {
            app.token_input.pop();
        }
        KeyCode::Char(c) => {
            app.token_input.push(c);
        }
        _ => {}
    }
}

fn handle_popup(app: &mut App, key: KeyEvent) {
    match app.popup {
        Popup::FieldPicker => match key.code {
            KeyCode::Esc => app.popup = Popup::None,
            KeyCode::Enter => app.confirm_field_pick(),
            KeyCode::Up | KeyCode::Char('k') => {
                if app.field_picker_idx > 0 {
                    app.field_picker_idx -= 1;
                }
            }
            KeyCode::Down | KeyCode::Char('j') => {
                let max = app.field_entries().len().saturating_sub(1);
                if app.field_picker_idx < max {
                    app.field_picker_idx += 1;
                }
            }
            _ => {}
        },
        Popup::StatusPicker => match key.code {
            KeyCode::Esc => app.popup = Popup::None,
            KeyCode::Enter => app.confirm_status_pick(),
            KeyCode::Up | KeyCode::Char('k') => {
                if app.status_picker_idx > 0 {
                    app.status_picker_idx -= 1;
                }
            }
            KeyCode::Down | KeyCode::Char('j') => {
                if app.status_picker_idx + 1 < app.status_options.len() {
                    app.status_picker_idx += 1;
                }
            }
            _ => {}
        },
        Popup::EditText => match key.code {
            KeyCode::Esc => app.popup = Popup::None,
            KeyCode::Enter => app.confirm_edit(),
            KeyCode::Backspace => {
                app.edit_buffer.pop();
            }
            KeyCode::Char(c) => app.edit_buffer.push(c),
            _ => {}
        },
        Popup::CreateItemGroup => match key.code {
            KeyCode::Esc => app.popup = Popup::None,
            KeyCode::Enter => app.confirm_create_group_pick(),
            KeyCode::Up | KeyCode::Char('k') => {
                if app.group_picker_idx > 0 {
                    app.group_picker_idx -= 1;
                }
            }
            KeyCode::Down | KeyCode::Char('j') => {
                let max = app
                    .current_board()
                    .map(|b| b.groups.len())
                    .unwrap_or(0)
                    .saturating_sub(1);
                if app.group_picker_idx < max {
                    app.group_picker_idx += 1;
                }
            }
            _ => {}
        },
        Popup::CreateItem => match key.code {
            KeyCode::Esc => app.popup = Popup::None,
            KeyCode::Enter => app.confirm_create_item(),
            KeyCode::Backspace => {
                app.create_buffer.pop();
            }
            KeyCode::Char(c) => app.create_buffer.push(c),
            _ => {}
        },
        Popup::ConfirmDelete => match key.code {
            KeyCode::Char('y') | KeyCode::Char('Y') => app.confirm_delete_item(),
            KeyCode::Esc | KeyCode::Char('n') | KeyCode::Char('N') => app.popup = Popup::None,
            _ => {}
        },
        Popup::Help => {
            if matches!(key.code, KeyCode::Esc | KeyCode::Char('?') | KeyCode::Char('q')) {
                app.popup = Popup::None;
            }
        }
        Popup::None => {}
    }
}

fn handle_screen(app: &mut App, key: KeyEvent) {
    match key.code {
        KeyCode::Char('q') => {
            app.should_quit = true;
            return;
        }
        KeyCode::Char('?') => {
            app.popup = Popup::Help;
            return;
        }
        KeyCode::Char('r') => {
            match app.screen {
                Screen::Workspaces => app.load_workspaces(),
                Screen::Boards => app.load_boards(),
                Screen::Items => app.load_items(),
                _ => {}
            }
            return;
        }
        _ => {}
    }

    match app.screen {
        Screen::Workspaces => handle_workspaces(app, key),
        Screen::Boards => handle_boards(app, key),
        Screen::Items => handle_items(app, key),
        Screen::TokenPrompt => {}
    }
}

fn handle_workspaces(app: &mut App, key: KeyEvent) {
    match key.code {
        KeyCode::Up | KeyCode::Char('k') => {
            if app.ws_idx > 0 {
                app.ws_idx -= 1;
            }
        }
        KeyCode::Down | KeyCode::Char('j') => {
            if app.ws_idx + 1 < app.workspaces.len() {
                app.ws_idx += 1;
            }
        }
        KeyCode::Home | KeyCode::Char('g') => app.ws_idx = 0,
        KeyCode::End | KeyCode::Char('G') => {
            app.ws_idx = app.workspaces.len().saturating_sub(1);
        }
        KeyCode::Enter => {
            if !app.workspaces.is_empty() {
                app.screen = Screen::Boards;
                app.load_boards();
            }
        }
        _ => {}
    }
}

fn handle_boards(app: &mut App, key: KeyEvent) {
    match key.code {
        KeyCode::Esc | KeyCode::Backspace => {
            app.screen = Screen::Workspaces;
        }
        KeyCode::Up | KeyCode::Char('k') => {
            if app.board_idx > 0 {
                app.board_idx -= 1;
            }
        }
        KeyCode::Down | KeyCode::Char('j') => {
            if app.board_idx + 1 < app.boards.len() {
                app.board_idx += 1;
            }
        }
        KeyCode::Home | KeyCode::Char('g') => app.board_idx = 0,
        KeyCode::End | KeyCode::Char('G') => {
            app.board_idx = app.boards.len().saturating_sub(1);
        }
        KeyCode::Enter => {
            if !app.boards.is_empty() {
                app.screen = Screen::Items;
                app.load_items();
            }
        }
        _ => {}
    }
}

fn handle_items(app: &mut App, key: KeyEvent) {
    match key.code {
        KeyCode::Esc | KeyCode::Backspace => {
            app.screen = Screen::Boards;
        }
        KeyCode::Up | KeyCode::Char('k') => {
            if app.item_idx > 0 {
                app.item_idx -= 1;
            }
        }
        KeyCode::Down | KeyCode::Char('j') => {
            if app.item_idx + 1 < app.items.len() {
                app.item_idx += 1;
            }
        }
        KeyCode::Home | KeyCode::Char('g') => app.item_idx = 0,
        KeyCode::End | KeyCode::Char('G') => {
            app.item_idx = app.items.len().saturating_sub(1);
        }
        KeyCode::Left | KeyCode::Char('h') => {
            if app.col_idx > 0 {
                app.col_idx -= 1;
            }
        }
        KeyCode::Right | KeyCode::Char('l') => {
            // 0 = Name, 1..=N = Board-Spalten
            let max = app.visible_columns().len();
            if app.col_idx < max {
                app.col_idx += 1;
            }
        }
        KeyCode::Char('s') => app.open_status_picker(),
        KeyCode::Char('e') => app.open_edit_text(),
        KeyCode::Char('n') => app.open_create_item(),
        KeyCode::Char('d') => {
            if app.current_item().is_some() {
                app.popup = Popup::ConfirmDelete;
            }
        }
        _ => {}
    }
}
