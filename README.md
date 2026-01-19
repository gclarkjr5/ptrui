# ptrui (Ratatui + Rust)

ptrui is a tiny terminal UI (TUI) written in Rust using Ratatui. It lets you type English on the left and see the Spanish translation on the right, or switch to Spanish and translate back to English, using a translation API.

## What you will learn

- How a Rust program is structured (`main`, functions, structs)
- How to build a TUI with Ratatui
- How to handle keyboard input with Crossterm
- How to keep application state in sync
- How to call an HTTP API from a TUI app

## Running the app

```bash
TRANSLATION_API_URL="https://api.deepl.com/v2/translate" \
TRANSLATION_API_KEY="your-key" \
TRANSLATION_API_AUTH_HEADER="DeepL-Auth-Key" \
cargo run
```

Environment variables:

- `TRANSLATION_API_URL` (required): API endpoint that accepts JSON `{ "text": ["..."], "source_lang": "...", "target_lang": "..." }`.
- `TRANSLATION_API_KEY` (optional): API key to send with requests.
- `TRANSLATION_API_AUTH_HEADER` (optional): Header name for the API key. Defaults to `Authorization` (Bearer).

Controls:

- `Tab` switches the active side (input focus)
- `Ctrl+c` quits
- `Ctrl+h` changes the left language
- `Ctrl+l` changes the right language
- `Ctrl+n` native-izes both sides
- `Ctrl+r` clears the active side
- `i` enters insert mode (Vim-style editing)

## Project layout

- `src/main.rs` contains all the code
- `Cargo.toml` lists dependencies

## How the code works (walkthrough)

### 1) State

The `App` struct stores the live state for the UI:

- `active` determines which side gets keyboard input
- `input` is the left (English) text
- `output` is the right (Spanish) text
- `pending_translation` tracks queued API calls
- `error` stores the last API error (if any)

### 2) Terminal setup

In `main`, the program:

- Enables raw mode (so key presses are available immediately)
- Enters an alternate screen (so the TUI does not overwrite your shell)
- Builds a `Terminal` with a Crossterm backend

When the app exits, it restores the terminal settings.

### 3) Event loop

`run_app` is the main loop. Each iteration:

1. Draws the UI (`draw_ui`)
2. Polls for keyboard input
3. Updates state based on the key pressed

Whenever you type or delete a character, the app schedules an API translation once input settles.

### 4) Rendering

The UI is built with Ratatui widgets:

- A header at the top
- Two side-by-side panels for translation
- A controls panel at the bottom

Ratatui layouts split the screen into rectangles (`Layout::split`), and each widget renders into a rectangle.

### 5) Translation

The app posts JSON to a translation API and expects a response shaped like:

```json
{ "translations": [{ "text": "..." }] }
```

## Suggested exercises

- Add a language selector
- Cache recent translations
- Show a "translating..." spinner next to the active pane

## Dependencies

- [ratatui](https://docs.rs/ratatui)
- [crossterm](https://docs.rs/crossterm)
- [reqwest](https://docs.rs/reqwest)
- [serde](https://docs.rs/serde)
