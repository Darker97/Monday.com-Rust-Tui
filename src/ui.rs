use crate::app::{App, Popup, RenderRow, Screen};
use ratatui::{
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Style, Stylize},
    text::{Line, Span},
    widgets::{
        Block, BorderType, Borders, Cell, Clear, List, ListItem, ListState, Paragraph, Row, Table,
        TableState, Wrap,
    },
    Frame,
};

const ACCENT: Color = Color::Cyan;
const HEADER: Color = Color::Magenta;
const OK: Color = Color::Green;
const WARN: Color = Color::Yellow;
const ERR: Color = Color::Red;

pub fn render(f: &mut Frame, app: &mut App) {
    let area = f.area();
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1), // Top-Bar
            Constraint::Min(3),    // Main
            Constraint::Length(1), // Bottom-Bar
        ])
        .split(area);

    render_top_bar(f, app, chunks[0]);
    render_main(f, app, chunks[1]);
    render_bottom_bar(f, app, chunks[2]);

    // Popups
    match app.popup {
        Popup::FieldPicker => render_field_picker(f, app, area),
        Popup::StatusPicker => render_status_picker(f, app, area),
        Popup::EditText => render_edit_popup(f, app, area),
        Popup::CreateItemGroup => render_create_group_picker(f, app, area),
        Popup::CreateItem => render_create_popup(f, app, area),
        Popup::ConfirmDelete => render_confirm_delete(f, app, area),
        Popup::Help => render_help(f, area),
        Popup::None => {}
    }

    // Token-Prompt überlagert alles
    if app.screen == Screen::TokenPrompt {
        render_token_prompt(f, app, area);
    }
}

fn render_top_bar(f: &mut Frame, app: &App, area: Rect) {
    let mut spans = vec![
        Span::styled(" Monday TUI ", Style::default().fg(Color::Black).bg(ACCENT).bold()),
        Span::raw(" "),
    ];
    // Breadcrumb
    let crumb = match app.screen {
        Screen::TokenPrompt => "setup".to_string(),
        Screen::Workspaces => "workspaces".to_string(),
        Screen::Boards => format!(
            "{} > boards",
            app.current_workspace().map(|w| w.name.as_str()).unwrap_or("")
        ),
        Screen::Items => format!(
            "{} > {}",
            app.current_workspace().map(|w| w.name.as_str()).unwrap_or(""),
            app.current_board().map(|b| b.name.as_str()).unwrap_or("")
        ),
    };
    spans.push(Span::styled(crumb, Style::default().fg(HEADER)));

    if app.loading {
        spans.push(Span::raw("  "));
        spans.push(Span::styled("● loading...", Style::default().fg(WARN)));
    }

    let p = Paragraph::new(Line::from(spans));
    f.render_widget(p, area);
}

fn render_bottom_bar(f: &mut Frame, app: &App, area: Rect) {
    // Toast hat Vorrang
    if let Some((msg, _)) = &app.error {
        let p = Paragraph::new(format!(" ✗ {}", msg)).style(Style::default().fg(ERR).bold());
        f.render_widget(p, area);
        return;
    }
    if let Some((msg, _)) = &app.info {
        let p = Paragraph::new(format!(" ✓ {}", msg)).style(Style::default().fg(OK));
        f.render_widget(p, area);
        return;
    }

    let hints = match (app.screen, app.popup) {
        (_, Popup::FieldPicker) => "[↑↓] pick field  [enter] edit  [esc] cancel",
        (_, Popup::StatusPicker) => "[↑↓] pick  [enter] confirm  [esc] cancel",
        (_, Popup::EditText) => "[enter] save  [esc] cancel",
        (_, Popup::CreateItemGroup) => "[↑↓] pick group  [enter] next  [esc] cancel",
        (_, Popup::CreateItem) => "[enter] create  [esc] cancel",
        (_, Popup::ConfirmDelete) => "[y] delete  [n/esc] cancel",
        (_, Popup::Help) => "[esc] close",
        (Screen::TokenPrompt, _) => "[enter] save token  [esc] quit",
        (Screen::Workspaces, _) => "[↑↓] nav  [enter] open  [r] reload  [?] help  [q] quit",
        (Screen::Boards, _) => "[↑↓] nav  [enter] open  [esc] back  [r] reload  [q] quit",
        (Screen::Items, _) => {
            "[↑↓] nav  [←→] col  [s] status  [e] edit  [n] new  [d] delete  [esc] back"
        }
    };
    let p = Paragraph::new(format!(" {}", hints))
        .style(Style::default().fg(Color::DarkGray));
    f.render_widget(p, area);
}

fn render_main(f: &mut Frame, app: &mut App, area: Rect) {
    match app.screen {
        Screen::TokenPrompt => {} // im Overlay gezeichnet
        Screen::Workspaces => render_workspaces(f, app, area),
        Screen::Boards => render_boards(f, app, area),
        Screen::Items => render_items(f, app, area),
    }
}

fn render_workspaces(f: &mut Frame, app: &mut App, area: Rect) {
    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .title(Span::styled(" Workspaces ", Style::default().fg(ACCENT).bold()));

    if app.workspaces.is_empty() {
        let msg = if app.loading { "Loading..." } else { "No workspaces. Press [r] to reload." };
        f.render_widget(
            Paragraph::new(msg)
                .block(block)
                .alignment(Alignment::Center),
            area,
        );
        return;
    }

    let items: Vec<ListItem> = app
        .workspaces
        .iter()
        .map(|w| {
            let kind_label = match w.kind.as_deref() {
                Some("main") => "★ main",
                Some("open") => "🌐 open",
                Some("closed") => "🔒 private",
                Some(other) => other,
                None => "",
            };
            let line = Line::from(vec![
                Span::raw(" "),
                Span::styled(&w.name, Style::default().bold()),
                Span::raw("  "),
                Span::styled(kind_label, Style::default().fg(Color::DarkGray)),
            ]);
            ListItem::new(line)
        })
        .collect();

    let list = List::new(items)
        .block(block)
        .highlight_style(Style::default().bg(ACCENT).fg(Color::Black).bold())
        .highlight_symbol("▶ ");

    let mut state = ListState::default();
    state.select(Some(app.ws_idx));
    f.render_stateful_widget(list, area, &mut state);
}

fn render_boards(f: &mut Frame, app: &mut App, area: Rect) {
    let title = format!(
        " Boards in \"{}\" ",
        app.current_workspace().map(|w| w.name.as_str()).unwrap_or("")
    );
    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .title(Span::styled(title, Style::default().fg(ACCENT).bold()));

    if app.boards.is_empty() {
        let msg = if app.loading { "Loading..." } else { "No boards." };
        f.render_widget(
            Paragraph::new(msg).block(block).alignment(Alignment::Center),
            area,
        );
        return;
    }

    let items: Vec<ListItem> = app
        .boards
        .iter()
        .map(|b| {
            let cols = b.columns.len();
            let count = b.items_count.map(|n| n.to_string()).unwrap_or_else(|| "—".into());
            let line = Line::from(vec![
                Span::raw(" "),
                Span::styled(&b.name, Style::default().bold()),
                Span::raw("  "),
                Span::styled(format!("{} cols", cols), Style::default().fg(Color::DarkGray)),
                Span::raw("  "),
                Span::styled(format!("{} items", count), Style::default().fg(Color::DarkGray)),
            ]);
            ListItem::new(line)
        })
        .collect();

    let list = List::new(items)
        .block(block)
        .highlight_style(Style::default().bg(ACCENT).fg(Color::Black).bold())
        .highlight_symbol("▶ ");

    let mut state = ListState::default();
    state.select(Some(app.board_idx));
    f.render_stateful_widget(list, area, &mut state);
}

fn render_items(f: &mut Frame, app: &mut App, area: Rect) {
    let title = format!(
        " {} ",
        app.current_board().map(|b| b.name.as_str()).unwrap_or("Items")
    );
    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .title(Span::styled(title, Style::default().fg(ACCENT).bold()));

    if app.items.is_empty() {
        let msg = if app.loading {
            "Loading..."
        } else {
            "No items yet — press [n] to create one."
        };
        f.render_widget(
            Paragraph::new(msg).block(block).alignment(Alignment::Center),
            area,
        );
        return;
    }

    let columns = app.visible_columns();
    let n_cols = columns.len();
    let plan = app.render_plan();

    // Header: Name (col_idx=0) + sichtbare Spalten (col_idx=1..=N)
    let name_style = if app.col_idx == 0 {
        Style::default().fg(HEADER).bold().bg(Color::DarkGray)
    } else {
        Style::default().fg(HEADER).bold()
    };
    let mut header_cells = vec![Cell::from("Name").style(name_style)];
    for (i, c) in columns.iter().enumerate() {
        let mut style = Style::default().fg(HEADER).bold();
        if i + 1 == app.col_idx {
            style = style.bg(Color::DarkGray);
        }
        header_cells.push(Cell::from(c.title.clone()).style(style));
    }

    // Render-Rows + Mapping item_idx -> Tabellen-Reihe (für Auswahl-Highlight)
    let mut rows: Vec<Row> = Vec::with_capacity(plan.len());
    let mut selected_table_row: Option<usize> = None;

    for (row_idx, entry) in plan.iter().enumerate() {
        match entry {
            RenderRow::Header { title, color, count } => {
                let group_color = monday_color(color.as_deref());
                let label = format!(" ▾ {}  ({})", title, count);
                let mut cells = vec![Cell::from(label).style(
                    Style::default().fg(group_color).bold().bg(Color::Rgb(20, 20, 28)),
                )];
                for _ in 0..n_cols {
                    cells.push(
                        Cell::from("").style(Style::default().bg(Color::Rgb(20, 20, 28))),
                    );
                }
                rows.push(Row::new(cells));
            }
            RenderRow::Item(idx) => {
                let item = &app.items[*idx];
                let group_color = item
                    .group
                    .as_ref()
                    .and_then(|g| g.id.as_ref())
                    .and_then(|gid| {
                        app.current_board()
                            .and_then(|b| b.groups.iter().find(|gg| &gg.id == gid))
                    })
                    .and_then(|g| g.color.clone());
                let bar = monday_color(group_color.as_deref());

                let name_cell = Cell::from(Line::from(vec![
                    Span::styled("▎", Style::default().fg(bar)),
                    Span::raw(" "),
                    Span::styled(item.name.clone(), Style::default().bold()),
                ]));
                let mut cells = vec![name_cell];
                for c in &columns {
                    let cv = item.value_for(&c.id);
                    let cell = match c.col_type.as_str() {
                        "status" => render_status_cell(cv),
                        _ => {
                            let txt = cv.and_then(|v| v.text.clone()).unwrap_or_default();
                            Cell::from(truncate(&txt, 30))
                        }
                    };
                    cells.push(cell);
                }
                if *idx == app.item_idx {
                    selected_table_row = Some(row_idx);
                }
                rows.push(Row::new(cells));
            }
        }
    }

    // Constraints: Name fix breit, dann gleichmäßig
    let mut constraints = vec![Constraint::Percentage(25)];
    let n = n_cols.max(1);
    let pct = (75 / n) as u16;
    for _ in 0..n_cols {
        constraints.push(Constraint::Percentage(pct));
    }

    let table = Table::new(rows, constraints)
        .header(Row::new(header_cells).bottom_margin(1))
        .block(block)
        .highlight_style(Style::default().bg(ACCENT).fg(Color::Black).bold())
        .highlight_symbol("▶ ");

    let mut state = TableState::default();
    state.select(selected_table_row);
    f.render_stateful_widget(table, area, &mut state);
}

fn render_status_cell(cv: Option<&crate::api::ColumnValue>) -> Cell<'static> {
    let text = cv.and_then(|v| v.text.clone()).unwrap_or_default();
    if text.is_empty() {
        return Cell::from("○ —").style(Style::default().fg(Color::DarkGray));
    }
    let color = match text.to_lowercase().as_str() {
        s if s.contains("done") || s.contains("erledigt") => OK,
        s if s.contains("working") || s.contains("progress") || s.contains("bearbeitung") => WARN,
        s if s.contains("stuck") || s.contains("blocked") || s.contains("blockiert") => ERR,
        _ => Color::Cyan,
    };
    Cell::from(format!("● {}", truncate(&text, 25))).style(Style::default().fg(color))
}

/// Mappt Monday Gruppen-Farben (oft Hex oder Namen) auf TUI-Farben.
fn monday_color(input: Option<&str>) -> Color {
    let s = match input {
        Some(s) if !s.is_empty() => s.to_lowercase(),
        _ => return Color::Cyan,
    };
    // Hex-Codes parsen (#RRGGBB)
    if let Some(stripped) = s.strip_prefix('#') {
        if stripped.len() == 6 {
            if let (Ok(r), Ok(g), Ok(b)) = (
                u8::from_str_radix(&stripped[0..2], 16),
                u8::from_str_radix(&stripped[2..4], 16),
                u8::from_str_radix(&stripped[4..6], 16),
            ) {
                return Color::Rgb(r, g, b);
            }
        }
    }
    // Bekannte Namen
    match s.as_str() {
        s if s.contains("green") => Color::Green,
        s if s.contains("red") => Color::Red,
        s if s.contains("blue") => Color::Blue,
        s if s.contains("yellow") || s.contains("gold") => Color::Yellow,
        s if s.contains("orange") => Color::Rgb(255, 165, 0),
        s if s.contains("purple") || s.contains("violet") => Color::Magenta,
        s if s.contains("pink") => Color::Rgb(255, 105, 180),
        s if s.contains("teal") => Color::Rgb(0, 192, 192),
        s if s.contains("grey") || s.contains("gray") => Color::DarkGray,
        s if s.contains("black") => Color::Black,
        _ => Color::Cyan,
    }
}

fn truncate(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        s.to_string()
    } else {
        let cut: String = s.chars().take(max.saturating_sub(1)).collect();
        format!("{}…", cut)
    }
}

// === Popups ===

fn centered_rect(percent_x: u16, percent_y: u16, r: Rect) -> Rect {
    let v = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage((100 - percent_y) / 2),
            Constraint::Percentage(percent_y),
            Constraint::Percentage((100 - percent_y) / 2),
        ])
        .split(r);
    Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage((100 - percent_x) / 2),
            Constraint::Percentage(percent_x),
            Constraint::Percentage((100 - percent_x) / 2),
        ])
        .split(v[1])[1]
}

fn render_status_picker(f: &mut Frame, app: &App, area: Rect) {
    let r = centered_rect(40, 50, area);
    f.render_widget(Clear, r);
    let items: Vec<ListItem> = app
        .status_options
        .iter()
        .map(|(_, label)| ListItem::new(format!("  {}", label)))
        .collect();
    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .title(Span::styled(" Change Status ", Style::default().fg(ACCENT).bold()));
    let list = List::new(items)
        .block(block)
        .highlight_style(Style::default().bg(ACCENT).fg(Color::Black).bold())
        .highlight_symbol("▶ ");
    let mut state = ListState::default();
    state.select(Some(app.status_picker_idx));
    f.render_stateful_widget(list, r, &mut state);
}

fn render_edit_popup(f: &mut Frame, app: &App, area: Rect) {
    let r = centered_rect(60, 30, area);
    f.render_widget(Clear, r);
    let col = app.edit_target_col.as_ref();
    let title = format!(
        " Edit: {} ",
        col.map(|c| c.title.as_str()).unwrap_or("")
    );
    let hint = col.map(|c| crate::app::format_hint(&c.col_type)).unwrap_or("");
    let type_info = col
        .map(|c| format!("type: {}", c.col_type))
        .unwrap_or_default();
    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .title(Span::styled(title, Style::default().fg(ACCENT).bold()));
    let lines = vec![
        Line::from(Span::styled(
            type_info,
            Style::default().fg(Color::DarkGray).italic(),
        )),
        Line::from(Span::styled(hint, Style::default().fg(Color::DarkGray))),
        Line::from(""),
        Line::from(format!("{}█", app.edit_buffer)),
    ];
    let p = Paragraph::new(lines).block(block).wrap(Wrap { trim: false });
    f.render_widget(p, r);
}

fn render_field_picker(f: &mut Frame, app: &App, area: Rect) {
    let r = centered_rect(70, 70, area);
    f.render_widget(Clear, r);
    let entries = app.field_entries();
    let items: Vec<ListItem> = entries
        .iter()
        .map(|e| {
            let editable = e.col.col_type == "status"
                || e.col.col_type == "color"
                || e.col.col_type == "checkbox"
                || crate::app::is_editable_simple(&e.col.col_type);
            let tag = if editable { "" } else { "  [read-only]" };
            let value_preview = if e.value.is_empty() { "—".to_string() } else { truncate(&e.value, 40) };
            let title_color = if editable { Color::White } else { Color::DarkGray };
            ListItem::new(Line::from(vec![
                Span::styled(
                    format!("{:<22}", truncate(&e.col.title, 20)),
                    Style::default().fg(title_color).bold(),
                ),
                Span::styled(
                    format!("{:<10}", e.col.col_type),
                    Style::default().fg(Color::DarkGray),
                ),
                Span::raw("  "),
                Span::raw(value_preview),
                Span::styled(tag, Style::default().fg(Color::DarkGray).italic()),
            ]))
        })
        .collect();
    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .title(Span::styled(
            " Edit field — pick column ",
            Style::default().fg(ACCENT).bold(),
        ));
    let list = List::new(items)
        .block(block)
        .highlight_style(Style::default().bg(ACCENT).fg(Color::Black).bold())
        .highlight_symbol("▶ ");
    let mut state = ListState::default();
    state.select(Some(app.field_picker_idx));
    f.render_stateful_widget(list, r, &mut state);
}

fn render_create_group_picker(f: &mut Frame, app: &App, area: Rect) {
    let r = centered_rect(50, 50, area);
    f.render_widget(Clear, r);
    let groups: Vec<ListItem> = app
        .current_board()
        .map(|b| {
            b.groups
                .iter()
                .map(|g| {
                    let color = monday_color(g.color.as_deref());
                    ListItem::new(Line::from(vec![
                        Span::styled("▎", Style::default().fg(color)),
                        Span::raw(" "),
                        Span::raw(g.title.clone()),
                    ]))
                })
                .collect()
        })
        .unwrap_or_default();
    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .title(Span::styled(
            " Create Item — pick group ",
            Style::default().fg(ACCENT).bold(),
        ));
    let list = List::new(groups)
        .block(block)
        .highlight_style(Style::default().bg(ACCENT).fg(Color::Black).bold())
        .highlight_symbol("▶ ");
    let mut state = ListState::default();
    state.select(Some(app.group_picker_idx));
    f.render_stateful_widget(list, r, &mut state);
}

fn render_create_popup(f: &mut Frame, app: &App, area: Rect) {
    let r = centered_rect(60, 20, area);
    f.render_widget(Clear, r);
    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .title(Span::styled(" Create New Item ", Style::default().fg(ACCENT).bold()));
    let p = Paragraph::new(format!("Name: {}█", app.create_buffer))
        .block(block)
        .wrap(Wrap { trim: false });
    f.render_widget(p, r);
}

fn render_confirm_delete(f: &mut Frame, app: &App, area: Rect) {
    let r = centered_rect(40, 20, area);
    f.render_widget(Clear, r);
    let name = app
        .current_item()
        .map(|i| i.name.clone())
        .unwrap_or_default();
    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(ERR))
        .title(Span::styled(" Delete Item? ", Style::default().fg(ERR).bold()));
    let p = Paragraph::new(format!("Delete \"{}\"?\n\nPress [y] to confirm, [n] to cancel.", name))
        .block(block)
        .wrap(Wrap { trim: true });
    f.render_widget(p, r);
}

fn render_help(f: &mut Frame, area: Rect) {
    let r = centered_rect(60, 70, area);
    f.render_widget(Clear, r);
    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .title(Span::styled(" Help ", Style::default().fg(ACCENT).bold()));
    let lines = vec![
        Line::from(Span::styled("Navigation", Style::default().fg(HEADER).bold())),
        Line::from("  ↑/k  ↓/j     move up/down"),
        Line::from("  ←/h  →/l     prev/next column (items view)"),
        Line::from("  g / G        jump to first / last"),
        Line::from("  Enter        open / select"),
        Line::from("  Esc          back / close popup"),
        Line::from(""),
        Line::from(Span::styled("Actions on items", Style::default().fg(HEADER).bold())),
        Line::from("  s            change status"),
        Line::from("  e            edit current cell (text/number/name)"),
        Line::from("  n            new item"),
        Line::from("  d            delete item"),
        Line::from(""),
        Line::from(Span::styled("Global", Style::default().fg(HEADER).bold())),
        Line::from("  r            reload"),
        Line::from("  ?            this help"),
        Line::from("  q            quit"),
    ];
    let p = Paragraph::new(lines).block(block);
    f.render_widget(p, r);
}

fn render_token_prompt(f: &mut Frame, app: &App, area: Rect) {
    let r = centered_rect(70, 40, area);
    f.render_widget(Clear, r);
    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .title(Span::styled(" Monday API Token ", Style::default().fg(ACCENT).bold()));
    let masked: String = "•".repeat(app.token_input.chars().count());
    let lines = vec![
        Line::from(""),
        Line::from(Span::styled(
            "Paste your Monday.com API token, then press Enter.",
            Style::default().fg(Color::DarkGray),
        )),
        Line::from(Span::styled(
            "Find it at: monday.com → profile → Developers → API token",
            Style::default().fg(Color::DarkGray),
        )),
        Line::from(""),
        Line::from(format!("Token: {}█", masked)),
        Line::from(""),
        Line::from(Span::styled(
            "Will be saved to ~/.config/monday-tui/config.toml",
            Style::default().fg(Color::DarkGray).italic(),
        )),
    ];
    let p = Paragraph::new(lines).block(block).wrap(Wrap { trim: false });
    f.render_widget(p, r);
}
