# Monday.com Rust TUI - Implementierungsplan

## Überblick

Eine Terminal-basierte Benutzeroberfläche (TUI) für Monday.com, die grundlegende Aufgabenverwaltung schnell und effizient ermöglicht. Gebaut mit Rust und dem ratatui Framework.

## Ziel

Schnelle Alternative zur überladenen Monday.com Web-Oberfläche für die Kern-Workflows: Aufgaben einsehen, anlegen und abarbeiten.

---

## Architektur

```
monday-tui/
├── Cargo.toml
├── src/
│   ├── main.rs              # Einstiegspunkt, Event-Loop
│   ├── app.rs               # App-State, Screen-Enum, Navigation
│   ├── config.rs            # Token laden/speichern
│   ├── api/
│   │   ├── mod.rs           # GraphQL Client (reqwest)
│   │   ├── queries.rs       # Query/Mutation-Strings
│   │   └── types.rs         # Workspace, Board, Item, Column structs
│   ├── ui/
│   │   ├── mod.rs           # render() Dispatcher
│   │   ├── workspaces.rs    # Workspace-Liste
│   │   ├── boards.rs        # Board-Liste
│   │   ├── items.rs         # Items-Tabelle
│   │   ├── popups.rs        # Status-Picker, Edit-Popup, Create-Popup
│   │   └── help.rs          # Help-Overlay
│   └── event.rs             # Async Event-Handling
└── Docu/
    └── plan.md
```

## Abhängigkeiten

| Crate | Zweck |
|-------|-------|
| `ratatui` | TUI-Rendering |
| `crossterm` | Terminal-Backend, Events |
| `tokio` | Async Runtime |
| `reqwest` | HTTP Client (rustls) |
| `serde` / `serde_json` | JSON-Parsing |
| `dirs` | Config-Pfad |
| `anyhow` | Error-Handling |
| `toml` | Config-Format |

---

## API-Integration (verifiziert gegen Monday.com Docs)

- **Endpoint:** `POST https://api.monday.com/v2`
- **Auth-Header:** `Authorization: <TOKEN>` (KEIN `Bearer ` Prefix)
- **API-Version:** `API-Version: 2024-01` Header empfohlen
- **Body:** `{ "query": "...", "variables": {...} }`

### Wichtige Queries

**Workspaces auflisten:**
```graphql
query { workspaces { id name kind description } }
```

**Boards eines Workspace:**
```graphql
query($wsId: [ID!]) {
  boards(workspace_ids: $wsId, limit: 50) {
    id name
    columns { id title type settings_str }
    groups { id title }
  }
}
```

**Items eines Boards (cursor-paginiert, max 500/Seite):**
```graphql
query($boardId: [ID!]) {
  boards(ids: $boardId) {
    items_page(limit: 100) {
      cursor
      items {
        id name
        column_values { id text value type ... on StatusValue { label index } }
      }
    }
  }
}
```

### Wichtige Mutations

**Status ändern (Index-basiert, robuster als Label):**
```graphql
mutation($boardId: ID!, $itemId: ID!, $colId: String!, $value: JSON!) {
  change_column_value(board_id: $boardId, item_id: $itemId,
                      column_id: $colId, value: $value) { id }
}
```
Wert: `"{\"index\": 1}"` (JSON-String, nicht Objekt!)

**Item anlegen:**
```graphql
mutation($boardId: ID!, $name: String!) {
  create_item(board_id: $boardId, item_name: $name) { id name }
}
```

### Status-Spalten-Settings parsen
`columns[].settings_str` enthält JSON mit Labels-Map: `{"labels": {"0": "Working on it", "1": "Done", ...}}`. Für Status-Picker parsen wir dieses Feld.

---

## Screens & Navigation

```
┌─────────────────────────────────────────────────┐
│  Workspaces                                     │
│       │                                         │
│   Enter ▼                                       │
│  Boards (eines Workspace)                       │
│       │                                         │
│   Enter ▼                                       │
│  Items (Tabellen-Ansicht eines Boards)          │
│       │                                         │
│   Aktionen: Status ändern, Edit, Neu, Delete    │
└─────────────────────────────────────────────────┘
```

`Esc` führt jeweils einen Screen zurück.

---

## UX & Design

### Farbpalette
- **Primär:** Cyan — Akzente, Auswahl-Highlights
- **Sekundär:** Magenta — Header
- **Erfolg:** Grün — "Done" Status, Erfolgsmeldungen
- **Warnung:** Gelb — "Working on it"
- **Fehler:** Rot — "Stuck", Fehlermeldungen
- **Neutral:** Gray — Inaktive Bereiche

### Layout

**Globales Layout (jeder Screen):**
```
┌─ Monday TUI ─ workspace > board ──────── [user]─┐
│                                                 │
│              MAIN AREA (variabler Inhalt)       │
│                                                 │
├─────────────────────────────────────────────────┤
│ [↑↓] navigate  [enter] open  [s] status  [q] quit│
└─────────────────────────────────────────────────┘
```

- **Top-Bar (1 Zeile):** App-Name, Breadcrumb-Pfad, User-Indikator
- **Main-Area:** Screen-spezifischer Inhalt
- **Bottom-Bar (1 Zeile):** Kontextuelle Tastatur-Hints

### Workspace-Screen
```
┌─ Workspaces ────────────────────────────────────┐
│ ▶ Marketing Team           open                 │
│   Engineering              open                 │
│   Personal Tasks           open                 │
│   ...                                           │
└─────────────────────────────────────────────────┘
```
- Vertikale Liste, Auswahl mit Highlight-Hintergrund
- Live-Anzeige der Workspace-Beschreibung in Footer-Bereich

### Board-Screen
```
┌─ Boards in "Marketing Team" ────────────────────┐
│ ▶ Q2 Campaigns         12 columns   45 items   │
│   Content Calendar     8 columns    23 items   │
│   ...                                           │
└─────────────────────────────────────────────────┘
```

### Items-Screen (Hauptarbeitsbereich)
```
┌─ Q2 Campaigns ──────────────────────────────────┐
│ Name              │ Status      │ Owner │ Date  │
│───────────────────┼─────────────┼───────┼───────│
│ ▶ Launch landing  │ ● Working   │ CB    │ 04.05 │
│   Email blast     │ ● Done      │ AS    │ 02.05 │
│   Press release   │ ○ Stuck     │ —     │ —     │
└─────────────────────────────────────────────────┘
```
- Status mit farbigem Bullet (●) basierend auf Status-Farbe
- Aktive Zeile hervorgehoben
- Spalten werden automatisch auf verfügbare Breite verteilt
- Scrolling vertikal & horizontal (bei vielen Spalten)

### Popups

**Status-Picker (Modal über Items-View):**
```
┌─ Change Status ─────────────┐
│   Working on it             │
│ ▶ Done                      │
│   Stuck                     │
│   (empty)                   │
│                             │
│ [enter] confirm  [esc] cancel│
└─────────────────────────────┘
```

**Edit-Popup für Text/Number:**
```
┌─ Edit: Item Name ───────────┐
│ Launch landing page█        │
│                             │
│ [enter] save   [esc] cancel │
└─────────────────────────────┘
```

**Create-Popup:**
```
┌─ Create New Item ───────────┐
│ Name: █                     │
│                             │
│ [enter] create [esc] cancel │
└─────────────────────────────┘
```

### Loading- & Error-States
- **Loading:** Spinner (`⠋⠙⠹⠸⠼⠴⠦⠧⠇⠏`) im Header bei API-Calls
- **Errors:** Rote Toast-Nachricht im Footer, 5 Sek sichtbar
- **Empty:** Zentrierter Hinweistext "No items yet — press [n] to create"

---

## Tastenbelegung

### Global
| Taste | Aktion |
|-------|--------|
| `q` / `Ctrl+C` | Beenden |
| `?` | Hilfe-Overlay |
| `r` | Aktuelle Daten neu laden |
| `Esc` | Zurück / Popup schließen |

### Listen-/Tabellen-Navigation
| Taste | Aktion |
|-------|--------|
| `↑` / `k` | Eine Zeile hoch |
| `↓` / `j` | Eine Zeile runter |
| `g` / `Home` | Zum Anfang |
| `G` / `End` | Zum Ende |
| `PgUp` / `PgDn` | Seitenweise |
| `Enter` | Auswählen / Öffnen |

### Items-Screen
| Taste | Aktion |
|-------|--------|
| `s` | Status ändern (Popup) |
| `e` | Aktuelles Feld bearbeiten |
| `n` | Neues Item anlegen |
| `d` | Item löschen (mit Bestätigung) |
| `→` / `l` | Nächste Spalte |
| `←` / `h` | Vorige Spalte |
| `/` | Suche/Filter |

---

## Konfiguration

Token-Pfad: `~/.config/monday-tui/config.toml`

```toml
[auth]
api_token = "eyJhbGciOi..."
```

Beim ersten Start: Token-Eingabe-Popup. Speichern nach Validierung.

Alternative: `MONDAY_API_TOKEN` Umgebungsvariable.

---

## Implementierungs-Phasen

### Phase 1 — Grundgerüst
- [x] Plan
- [ ] Cargo-Projekt + Dependencies
- [ ] Config-Loader (Token aus File/Env)
- [ ] Terminal-Setup (raw mode, alternate screen)
- [ ] Event-Loop (crossterm + tokio)
- [ ] Basic App-State + Screen-Enum

### Phase 2 — API-Client
- [ ] GraphQL POST-Client
- [ ] Workspace-Query
- [ ] Boards-Query
- [ ] Items-Query (mit Pagination)

### Phase 3 — Read-Only TUI
- [ ] Workspace-Liste rendern + navigieren
- [ ] Board-Liste rendern + navigieren
- [ ] Items-Tabelle mit Spalten rendern
- [ ] Loading- und Error-States

### Phase 4 — Edit-Funktionen
- [ ] Status-Picker-Popup mit Live-Mutation
- [ ] Text-Feld-Edit-Popup
- [ ] Item-Create-Popup

### Phase 5 — Polish
- [ ] Help-Overlay
- [ ] Suche/Filter
- [ ] Bessere Fehlermeldungen
- [ ] Spinner-Animation

---

## Korrektheits-Check (vs. Monday.com API Docs)

| Aspekt | Status |
|--------|--------|
| Endpoint URL `/v2` | ✓ |
| Auth ohne `Bearer` Prefix | ✓ |
| `items_page` statt deprecated `items` | ✓ |
| Cursor-basierte Pagination | ✓ |
| Status via `index` (zuverlässiger als `label`) | ✓ |
| `value` als JSON-String, nicht Objekt | ✓ |
| Rate-Limit-Bewusstsein (10M Complexity/min) | ✓ |
| Column `settings_str` für Status-Labels parsen | ✓ |
