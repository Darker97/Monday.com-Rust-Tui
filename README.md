# monday-tui

A fast, keyboard-driven terminal UI for [Monday.com](https://monday.com) — built because the web app is too heavy when all you really do is triage and update tasks.

Built with Rust + [ratatui](https://ratatui.rs).

## What it does

- Browse the workspaces you actually work in (derived from your accessible boards — no noise from empty/private ones)
- List boards per workspace
- Show items grouped by their Monday group, with colored group bars
- Quickly change a status column via picker
- Edit any supported column (text, numbers, date, timeline, email, phone, link, dropdown, checkbox, …) via a field picker
- Create new items in a chosen group
- Delete items
- Reload on demand

Read-only column types (people, formula, mirror, timeline of files, …) are clearly marked and skipped during editing.

## Installation

Requires Rust 1.75+ (`rustup` recommended).

```bash
git clone <this-repo>
cd 146-Monday.com\ Rust\ Tui
cargo build --release
```

The binary is then at `target/release/monday-tui`. Optionally install it to `~/.cargo/bin`:

```bash
cargo install --path .
```

## Configuration

You need a Monday.com API token. Get one at:
**monday.com → profile picture → Developers → API token**

Provide it either via:

- **Env var** (one-shot): `MONDAY_API_TOKEN=eyJ... monday-tui`
- **Config file** (persistent): on first start the TUI prompts for the token and writes it to `~/.config/monday-tui/config.toml`.

## Usage

```bash
monday-tui
```

Navigation is keyboard-only. Press `?` at any time for the help overlay.

### Global

| Key | Action |
|---|---|
| `q` / `Ctrl+C` | Quit |
| `?` | Help overlay |
| `r` | Reload current view |
| `Esc` / `Backspace` | Back / close popup |

### Lists & tables

| Key | Action |
|---|---|
| `↑` / `k`, `↓` / `j` | Move selection |
| `g` / `Home`, `G` / `End` | Jump to first / last |
| `Enter` | Open / confirm |

### Items view

| Key | Action |
|---|---|
| `←` / `h`, `→` / `l` | Highlight previous / next column |
| `s` | Change status (picker) |
| `e` | Edit a field — opens a field picker showing all columns and current values |
| `n` | New item — pick group, then enter name |
| `d` | Delete item (with confirmation) |

### Field-edit format hints

The edit popup shows the expected format per column type:

- `date` → `YYYY-MM-DD`
- `timeline` → `YYYY-MM-DD YYYY-MM-DD` (start end; empty clears)
- `numbers` → e.g. `42` or `3.14`
- `email` → `address@example.com  Display Name`
- `phone` → `+491701234567  DE`
- `link` → `https://url  Display`
- `dropdown` → comma-separated labels
- `checkbox` → toggles directly, no popup
- `hour` → `HH:MM` (24h)
- `rating` → `1`–`5`

## Project structure

```
src/
├── main.rs        # Entry point, event loop, key routing
├── app.rs         # State, screens, popups, mutations
├── config.rs      # Token load/save
├── api/
│   ├── mod.rs     # GraphQL client + queries/mutations
│   └── types.rs   # Workspace / Board / Item / Column structs
└── ui.rs          # ratatui rendering for all screens & popups

Docu/
└── plan.md        # Original design / UX plan
```

## License

MIT (or whatever you prefer — adapt as needed).
