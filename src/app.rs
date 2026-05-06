use crate::api::{
    parse_status_labels, Board, Column, Group, Item, MondayClient, Workspace,
};
use anyhow::Result;
use std::sync::Arc;
use std::time::Instant;
use tokio::sync::mpsc;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Screen {
    TokenPrompt,
    Workspaces,
    Boards,
    Items,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Popup {
    None,
    FieldPicker,     // 'e' → wähle Feld zum Editieren
    StatusPicker,
    EditText,
    CreateItemGroup, // Schritt 1: Gruppe wählen
    CreateItem,      // Schritt 2: Name eingeben
    ConfirmDelete,
    Help,
}

#[derive(Debug)]
pub enum Msg {
    OverviewLoaded(Result<Vec<Board>>),
    ItemsLoaded(Result<Vec<Item>>),
    Mutated(Result<()>),
    ItemCreated(Result<Item>),
}

pub struct App {
    pub client: Arc<MondayClient>,
    pub tx: mpsc::UnboundedSender<Msg>,

    pub screen: Screen,
    pub popup: Popup,
    pub should_quit: bool,
    pub loading: bool,
    pub error: Option<(String, Instant)>,
    pub info: Option<(String, Instant)>,

    // Token-Eingabe
    pub token_input: String,

    // Daten
    pub workspaces: Vec<Workspace>,
    pub boards: Vec<Board>,
    pub all_boards: Vec<Board>, // Cache: alle Boards des Users
    pub items: Vec<Item>,

    // Selektion
    pub ws_idx: usize,
    pub board_idx: usize,
    pub item_idx: usize,
    pub col_idx: usize, // Selektierte Spalte in Items-Tabelle

    // Aktive IDs
    pub current_ws_id: Option<String>,
    pub current_board_id: Option<String>,

    // Popup-State
    pub status_options: Vec<(i64, String)>,
    pub status_picker_idx: usize,
    pub status_target_col: Option<String>,
    pub edit_buffer: String,
    pub edit_target_col: Option<Column>,
    pub create_buffer: String,

    // Group-Picker für Create
    pub group_picker_idx: usize,
    pub pending_create_group: Option<String>,

    // Field-Picker (zum Editieren)
    pub field_picker_idx: usize,
}

impl App {
    pub fn new(client: MondayClient, tx: mpsc::UnboundedSender<Msg>, has_token: bool) -> Self {
        Self {
            client: Arc::new(client),
            tx,
            screen: if has_token {
                Screen::Workspaces
            } else {
                Screen::TokenPrompt
            },
            popup: Popup::None,
            should_quit: false,
            loading: false,
            error: None,
            info: None,
            token_input: String::new(),
            workspaces: Vec::new(),
            boards: Vec::new(),
            all_boards: Vec::new(),
            items: Vec::new(),
            ws_idx: 0,
            board_idx: 0,
            item_idx: 0,
            col_idx: 0,
            current_ws_id: None,
            current_board_id: None,
            status_options: Vec::new(),
            status_picker_idx: 0,
            status_target_col: None,
            edit_buffer: String::new(),
            edit_target_col: None,
            create_buffer: String::new(),
            group_picker_idx: 0,
            pending_create_group: None,
            field_picker_idx: 0,
        }
    }

    pub fn set_error(&mut self, msg: impl Into<String>) {
        self.error = Some((msg.into(), Instant::now()));
    }

    pub fn set_info(&mut self, msg: impl Into<String>) {
        self.info = Some((msg.into(), Instant::now()));
    }

    pub fn clear_old_toasts(&mut self) {
        if let Some((_, t)) = &self.error {
            if t.elapsed().as_secs() > 6 {
                self.error = None;
            }
        }
        if let Some((_, t)) = &self.info {
            if t.elapsed().as_secs() > 4 {
                self.info = None;
            }
        }
    }

    pub fn current_workspace(&self) -> Option<&Workspace> {
        self.workspaces.get(self.ws_idx)
    }

    pub fn current_board(&self) -> Option<&Board> {
        self.boards.get(self.board_idx)
    }

    pub fn current_item(&self) -> Option<&Item> {
        self.items.get(self.item_idx)
    }

    /// Spalten für die Items-Tabelle (max. 5 sichtbar, scrollbar via col_idx)
    pub fn visible_columns(&self) -> Vec<&Column> {
        if let Some(b) = self.current_board() {
            b.columns.iter().take(20).collect()
        } else {
            Vec::new()
        }
    }

    /// Liefert eine Render-Reihenfolge: pro Gruppe ein Header, dann ihre Items.
    /// `item_idx` adressiert weiterhin `self.items` (nicht die Render-Position).
    pub fn render_plan(&self) -> Vec<RenderRow> {
        let board = match self.current_board() {
            Some(b) => b,
            None => return Vec::new(),
        };
        let mut plan: Vec<RenderRow> = Vec::new();
        // Reihenfolge der Gruppen vom Board
        let mut order: Vec<&Group> = board.groups.iter().collect();
        // Falls Items eine Gruppe haben, die nicht in board.groups steht (selten),
        // sammeln wir sie zusätzlich am Ende.
        let known: std::collections::HashSet<&str> =
            order.iter().map(|g| g.id.as_str()).collect();
        let mut extra: Vec<(String, String)> = Vec::new();
        for it in &self.items {
            if let Some(g) = &it.group {
                if let Some(id) = &g.id {
                    if !known.contains(id.as_str())
                        && !extra.iter().any(|(eid, _)| eid == id)
                    {
                        extra.push((id.clone(), g.title.clone().unwrap_or_default()));
                    }
                }
            }
        }
        // Pseudo-Group für Items ohne Gruppe
        let mut ungrouped: Vec<usize> = Vec::new();
        for g in &order {
            let title = g.title.clone();
            let color = g.color.clone();
            let items: Vec<usize> = self
                .items
                .iter()
                .enumerate()
                .filter(|(_, it)| {
                    it.group
                        .as_ref()
                        .and_then(|x| x.id.as_deref())
                        .map(|id| id == g.id)
                        .unwrap_or(false)
                })
                .map(|(i, _)| i)
                .collect();
            if items.is_empty() {
                continue;
            }
            plan.push(RenderRow::Header {
                title,
                color,
                count: items.len(),
            });
            for i in items {
                plan.push(RenderRow::Item(i));
            }
        }
        for (id, title) in &extra {
            let items: Vec<usize> = self
                .items
                .iter()
                .enumerate()
                .filter(|(_, it)| it.group.as_ref().and_then(|x| x.id.as_deref()) == Some(id))
                .map(|(i, _)| i)
                .collect();
            if items.is_empty() {
                continue;
            }
            plan.push(RenderRow::Header {
                title: title.clone(),
                color: None,
                count: items.len(),
            });
            for i in items {
                plan.push(RenderRow::Item(i));
            }
        }
        for (i, it) in self.items.iter().enumerate() {
            if it.group.is_none() {
                ungrouped.push(i);
            }
        }
        if !ungrouped.is_empty() {
            plan.push(RenderRow::Header {
                title: "(no group)".to_string(),
                color: None,
                count: ungrouped.len(),
            });
            for i in ungrouped {
                plan.push(RenderRow::Item(i));
            }
        }
        // Falls die Boards keine Gruppen haben, einfach alle Items flach rendern
        if order.is_empty() && extra.is_empty() && plan.is_empty() {
            for i in 0..self.items.len() {
                plan.push(RenderRow::Item(i));
            }
        }
        plan
    }
}

/// Welche Spaltentypen via "simple value" Mutation editierbar sind.
pub fn is_editable_simple(col_type: &str) -> bool {
    matches!(
        col_type,
        "text"
            | "long_text"
            | "numbers"
            | "name"
            | "email"
            | "phone"
            | "link"
            | "world_clock"
            | "country"
            | "date"
            | "timeline"
            | "checkbox"
            | "dropdown"
            | "hour"
            | "rating"
    )
}

/// Normalisiert einen vom User eingegebenen Wert in das Format,
/// das `change_simple_column_value` erwartet.
pub fn normalize_simple_value(col_type: &str, raw: &str) -> String {
    let trimmed = raw.trim();
    match col_type {
        "timeline" => {
            // Akzeptiere "YYYY-MM-DD YYYY-MM-DD", "YYYY-MM-DD - YYYY-MM-DD",
            // "YYYY-MM-DD,YYYY-MM-DD". Sende immer "YYYY-MM-DD YYYY-MM-DD".
            if trimmed.is_empty() {
                return String::new();
            }
            let cleaned = trimmed
                .replace(" - ", " ")
                .replace('-', "-") // platzhalter, no-op (Datums-Trenner bleibt)
                ;
            // Wenn der String "YYYY-MM-DD - YYYY-MM-DD" war, ist das schon ersetzt.
            // Außerdem: Komma → Leerzeichen.
            let cleaned = cleaned.replace(',', " ");
            // Mehrfache Leerzeichen zu einem
            let parts: Vec<&str> = cleaned.split_whitespace().collect();
            if parts.len() == 2 {
                format!("{} {}", parts[0], parts[1])
            } else {
                cleaned.split_whitespace().collect::<Vec<_>>().join(" ")
            }
        }
        _ => trimmed.to_string(),
    }
}

/// Format-Hint für Edit-Popup pro Spaltentyp.
pub fn format_hint(col_type: &str) -> &'static str {
    match col_type {
        "date" => "Format: YYYY-MM-DD  (z.B. 2026-05-04)",
        "timeline" => "Format: YYYY-MM-DD YYYY-MM-DD  (Start Ende, leeres Feld = clear)",
        "numbers" => "Number, e.g. 42 or 3.14",
        "email" => "Format: address@example.com  Display Name",
        "phone" => "Format: +491701234567  DE",
        "link" => "Format: https://url.example  Display",
        "dropdown" => "Comma-separated labels, e.g. Marketing,Design",
        "hour" => "Format: HH:MM  (24h)",
        "rating" => "Number 1-5",
        "world_clock" => "Timezone, e.g. Europe/Berlin",
        "country" => "Country code, e.g. DE",
        "long_text" => "Multi-line text",
        "name" => "Item name",
        _ => "Plain text",
    }
}

#[derive(Debug, Clone)]
pub struct FieldEntry {
    pub col: Column,
    pub value: String,
}

#[derive(Debug, Clone)]
pub enum RenderRow {
    Header {
        title: String,
        color: Option<String>,
        count: usize,
    },
    Item(usize),
}

impl App {

    /// Holt einmal alle Boards des Users und leitet daraus Workspaces ab.
    pub fn load_workspaces(&mut self) {
        self.loading = true;
        let client = self.client.clone();
        let tx = self.tx.clone();
        tokio::spawn(async move {
            let res = client.fetch_boards_overview().await;
            let _ = tx.send(Msg::OverviewLoaded(res));
        });
    }

    /// Filtert die gecachten Boards nach aktuellem Workspace — kein API-Call nötig.
    pub fn load_boards(&mut self) {
        let Some(ws) = self.current_workspace() else {
            return;
        };
        let ws_id = ws.id_str();
        self.current_ws_id = Some(ws_id.clone());
        self.boards = MondayClient::filter_boards_for_workspace(&self.all_boards, &ws_id);
        self.board_idx = 0;
    }

    pub fn load_items(&mut self) {
        let Some(b) = self.current_board() else {
            return;
        };
        let board_id = b.id_str();
        self.current_board_id = Some(board_id.clone());
        self.loading = true;
        self.items.clear();
        self.item_idx = 0;
        self.col_idx = 0;
        let client = self.client.clone();
        let tx = self.tx.clone();
        tokio::spawn(async move {
            let res = client.list_items(&board_id).await;
            let _ = tx.send(Msg::ItemsLoaded(res));
        });
    }

    pub fn handle_msg(&mut self, msg: Msg) {
        self.loading = false;
        match msg {
            Msg::OverviewLoaded(Ok(boards)) => {
                self.workspaces = MondayClient::derive_workspaces(&boards);
                self.all_boards = boards;
                if self.ws_idx >= self.workspaces.len() {
                    self.ws_idx = self.workspaces.len().saturating_sub(1);
                }
            }
            Msg::OverviewLoaded(Err(e)) => self.set_error(format!("Overview: {}", e)),
            Msg::ItemsLoaded(Ok(i)) => {
                self.items = i;
            }
            Msg::ItemsLoaded(Err(e)) => self.set_error(format!("Items: {}", e)),
            Msg::Mutated(Ok(())) => {
                self.set_info("Saved.");
                // Items neu laden, um Werte zu aktualisieren
                if self.screen == Screen::Items {
                    self.load_items();
                }
            }
            Msg::Mutated(Err(e)) => self.set_error(format!("Save failed: {}", e)),
            Msg::ItemCreated(Ok(_)) => {
                self.set_info("Item created.");
                self.load_items();
            }
            Msg::ItemCreated(Err(e)) => self.set_error(format!("Create failed: {}", e)),
        }
    }

    // === Popup-Triggers ===

    pub fn open_status_picker(&mut self) {
        let col_id_settings: Option<(String, String)> = {
            let Some(b) = self.current_board() else {
                return;
            };
            let cols = &b.columns;
            // col_idx 0 = Name → kein Status; sonst board-spalte = col_idx-1
            let board_col_idx = self.col_idx.checked_sub(1);
            let target = board_col_idx
                .and_then(|i| cols.get(i))
                .filter(|c| c.col_type == "status")
                .or_else(|| cols.iter().find(|c| c.col_type == "status"));
            target.map(|c| (c.id.clone(), c.settings_str.clone().unwrap_or_default()))
        };
        let Some((col_id, settings)) = col_id_settings else {
            self.set_error("No status column on this board.");
            return;
        };
        let labels = parse_status_labels(&settings);
        if labels.is_empty() {
            self.set_error("No status labels configured.");
            return;
        }
        self.status_options = labels;
        self.status_target_col = Some(col_id);
        self.status_picker_idx = 0;
        self.popup = Popup::StatusPicker;
    }

    pub fn confirm_status_pick(&mut self) {
        let Some((idx, _)) = self.status_options.get(self.status_picker_idx).cloned() else {
            return;
        };
        let Some(col_id) = self.status_target_col.clone() else {
            return;
        };
        let Some(item) = self.current_item() else {
            return;
        };
        let Some(board_id) = self.current_board_id.clone() else {
            return;
        };
        let item_id = item.id_str();
        self.popup = Popup::None;
        let client = self.client.clone();
        let tx = self.tx.clone();
        tokio::spawn(async move {
            let res = client.change_status(&board_id, &item_id, &col_id, idx).await;
            let _ = tx.send(Msg::Mutated(res));
        });
    }

    /// Liefert alle Felder (Name + Board-Spalten) des aktuellen Items mit aktuellem Wert.
    pub fn field_entries(&self) -> Vec<FieldEntry> {
        let mut out = Vec::new();
        let item = match self.current_item() {
            Some(i) => i,
            None => return out,
        };
        // Name als erstes Feld
        out.push(FieldEntry {
            col: Column {
                id: "name".to_string(),
                title: "Name".to_string(),
                col_type: "name".to_string(),
                settings_str: None,
            },
            value: item.name.clone(),
        });
        if let Some(b) = self.current_board() {
            for c in &b.columns {
                let value = item
                    .value_for(&c.id)
                    .and_then(|v| v.text.clone())
                    .unwrap_or_default();
                out.push(FieldEntry {
                    col: c.clone(),
                    value,
                });
            }
        }
        out
    }

    /// 'e' öffnet jetzt zuerst den Field-Picker.
    pub fn open_edit_text(&mut self) {
        if self.current_item().is_none() {
            self.set_error("No item selected.");
            return;
        }
        self.field_picker_idx = self.col_idx.min(self.field_entries().len().saturating_sub(1));
        self.popup = Popup::FieldPicker;
    }

    /// Wird aus dem FieldPicker aufgerufen.
    pub fn confirm_field_pick(&mut self) {
        let entries = self.field_entries();
        let Some(entry) = entries.get(self.field_picker_idx).cloned() else {
            return;
        };
        let col = entry.col;
        // Status → Status-Picker
        if col.col_type == "status" || col.col_type == "color" {
            // col_idx so setzen, dass open_status_picker diese Spalte trifft
            if let Some(idx) = self
                .current_board()
                .and_then(|b| b.columns.iter().position(|c| c.id == col.id))
            {
                self.col_idx = idx + 1;
            }
            self.open_status_picker();
            return;
        }
        // Checkbox → toggeln
        if col.col_type == "checkbox" {
            let current_checked = self
                .current_item()
                .and_then(|i| i.value_for(&col.id))
                .and_then(|v| v.text.clone())
                .map(|t| !t.is_empty() && t != "false" && t != "0")
                .unwrap_or(false);
            self.popup = Popup::None;
            self.toggle_checkbox(&col.id, !current_checked);
            return;
        }
        // Editierbar via simple-value
        if !is_editable_simple(&col.col_type) {
            self.set_error(format!(
                "'{}' (type {}) is read-only via simple edit.",
                col.title, col.col_type
            ));
            self.popup = Popup::None;
            return;
        }
        // Timeline: Bindestrich-Format auf Leerzeichen-Format normalisieren
        self.edit_buffer = match col.col_type.as_str() {
            "timeline" => entry.value.replace(" - ", " "),
            _ => entry.value,
        };
        self.edit_target_col = Some(col);
        self.popup = Popup::EditText;
    }

    fn toggle_checkbox(&mut self, col_id: &str, checked: bool) {
        let Some(item) = self.current_item() else {
            return;
        };
        let Some(board_id) = self.current_board_id.clone() else {
            return;
        };
        let item_id = item.id_str();
        let col_id = col_id.to_string();
        let value = if checked { "v".to_string() } else { String::new() };
        let client = self.client.clone();
        let tx = self.tx.clone();
        tokio::spawn(async move {
            let res = client
                .change_simple(&board_id, &item_id, &col_id, &value)
                .await;
            let _ = tx.send(Msg::Mutated(res));
        });
    }

    pub fn confirm_edit(&mut self) {
        let Some(col) = self.edit_target_col.clone() else {
            return;
        };
        let Some(item) = self.current_item() else {
            return;
        };
        let Some(board_id) = self.current_board_id.clone() else {
            return;
        };
        let item_id = item.id_str();
        let value = normalize_simple_value(&col.col_type, &self.edit_buffer);
        let col_id = if col.col_type == "name" {
            "name".to_string()
        } else {
            col.id.clone()
        };
        self.popup = Popup::None;
        let client = self.client.clone();
        let tx = self.tx.clone();
        tokio::spawn(async move {
            let res = client
                .change_simple(&board_id, &item_id, &col_id, &value)
                .await;
            let _ = tx.send(Msg::Mutated(res));
        });
    }

    pub fn open_create_item(&mut self) {
        if self.current_board_id.is_none() {
            self.set_error("No board selected.");
            return;
        }
        self.create_buffer.clear();
        self.pending_create_group = None;
        // Falls die aktuell ausgewählte Item-Zeile eine Gruppe hat, vorbelegen
        let preselect_idx = self
            .current_item()
            .and_then(|i| i.group.as_ref())
            .and_then(|g| g.id.as_ref())
            .and_then(|gid| {
                self.current_board()
                    .map(|b| b.groups.iter().position(|g| &g.id == gid))
                    .flatten()
            })
            .unwrap_or(0);
        self.group_picker_idx = preselect_idx;
        // Wenn Board keine Gruppen hat, direkt zur Name-Eingabe
        let groups_empty = self
            .current_board()
            .map(|b| b.groups.is_empty())
            .unwrap_or(true);
        if groups_empty {
            self.popup = Popup::CreateItem;
        } else {
            self.popup = Popup::CreateItemGroup;
        }
    }

    pub fn confirm_create_group_pick(&mut self) {
        let Some(b) = self.current_board() else {
            return;
        };
        let Some(g) = b.groups.get(self.group_picker_idx) else {
            return;
        };
        self.pending_create_group = Some(g.id.clone());
        self.popup = Popup::CreateItem;
    }

    pub fn confirm_create_item(&mut self) {
        let name = self.create_buffer.trim().to_string();
        if name.is_empty() {
            self.popup = Popup::None;
            return;
        }
        let Some(board_id) = self.current_board_id.clone() else {
            return;
        };
        let group_id = self.pending_create_group.clone();
        self.popup = Popup::None;
        let client = self.client.clone();
        let tx = self.tx.clone();
        tokio::spawn(async move {
            let res = client
                .create_item_in_group(&board_id, group_id.as_deref(), &name)
                .await;
            let _ = tx.send(Msg::ItemCreated(res));
        });
    }

    pub fn confirm_delete_item(&mut self) {
        let Some(item) = self.current_item() else {
            return;
        };
        let item_id = item.id_str();
        self.popup = Popup::None;
        let client = self.client.clone();
        let tx = self.tx.clone();
        tokio::spawn(async move {
            let res = client.delete_item(&item_id).await;
            let _ = tx.send(Msg::Mutated(res));
        });
    }
}
