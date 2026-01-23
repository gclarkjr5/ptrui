# ptrui (Translation Tool)

ptrui is a tiny terminal translation tool with a two-pane workflow for live, bidirectional translation. It is written in Rust using Ratatui and a translation API.

## Features

- Bidirectional translation with independent source/target panes
- Vim-style editing modes (normal/insert/visual) with familiar motions
- Language picker with fuzzy search for both panes
- Debounced API calls with live status ("translating", "ready", errors)
- Clear active pane or "native-ize" both sides on demand
- Configurable auth header and key for API requests

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

## Dependencies

- [ratatui](https://docs.rs/ratatui)
- [crossterm](https://docs.rs/crossterm)
- [reqwest](https://docs.rs/reqwest)
- [serde](https://docs.rs/serde)

## Release workflow

Release tags are the trigger for producing a release. Use the following format and steps.

Release format (required):

- Git tag format: `vX.Y.Z` (for example `v0.2.0`)
- `Cargo.toml` version must match `X.Y.Z`

Steps:

1. Update the version in `Cargo.toml`.
2. Run `cargo build` (or `cargo test`) to refresh `Cargo.lock` if needed.
3. Commit the version bump.
4. Create an annotated tag matching the version, for example: `git tag -a v0.2.0 -m "v0.2.0"`.
5. Push the commit and tag: `git push && git push --tags`.
