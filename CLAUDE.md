# QuoteCraft — instructions for agents

QuoteCraft is an open-source, Rust-native quoting tool for a telecom operator's sales team:
price lists (connectivity, hardware, services; direct and wholesale prices), quotes for direct
customers and the wholesale "Allegato A", the documents and the mail through the sibling apps.
It is part of the *craft* family (`covercraft`, `mailcraft`, `wordcraft`, …) and follows the
same conventions.

## Division of labour
- QuoteCraft owns the data (`data.json`), the quotes' content (HTML, placeholder fields) and
  the workflow. It makes **no DOCX/PDF and sends no mail itself**.
- **WordCraft** (`wordcraft-cli`) makes the DOCX and PDF: `convert quote.html …` for the
  built-in layout and HTML letterheads, `run --file letterhead.docx --cmd edit.replaceAll…
  --cmd insert.textFromFile… --cmd file.exportPdf… --save …` for DOCX letterheads
  (`crates/bridge/src/wordcraft.rs`).
- **MailCraft** (`POST /api/cmd` `mail.send`, service token + `X-Mailcraft-User`) sends the
  mail from the user's own account (`crates/bridge/src/mailcraft.rs`).
- Both are injected into the engine as `core::bridge::{Renderer, Sender}` factories; tests use
  doubles, the web build has none.

## Never crash
- No `unwrap()`, `expect()`, `panic!`, `unreachable!`, `todo!`, `unimplemented!` or `unsafe` in
  non-test code (crates carry `#![deny(clippy::unwrap_used, …)]`, `unsafe_code = "forbid"`).
- Errors are `Result` through each crate's error type (`core::Error`, `engine::Error`).
- Input is hostile: command and MCP parameters, the data file, letterheads, file names.
  Bounded lists (`core::limits`), `get()` over indexing, checked money arithmetic
  (`core::money::Cents`), template names are plain file names (`bridge::template_path`).
- The worker catches a panicking job; `Session::execute` catches a panicking command.
- Every crash fix lands with a regression test. `every_command_survives_empty_and_hostile_params`
  guards the engine.

## Rights
- Changing the price list, the letterheads, the company data and the settings needs an
  **administrator** (`Command::admin`, `Session::require_admin`). The server takes the role
  from the framework (`X-Quotecraft-Role: admin`); a local desktop/CLI session is its owner's
  and is an administrator; `--everyone-admin` for a single-user server. Language and theme are
  anyone's. Reading, customers and quotes are everyone's.
- The data is one store shared by every user's session (`SharedStore`); each command locks it
  briefly and saves atomically.

## Architecture
- Layers (checked by `cargo xtask layers`): `core` (L0) → `bridge` (L1) → `engine` (L5) →
  `ui-egui` / `mcp` / `remote` (L6) → `apps/*` (L7). Nothing below L6 depends on egui; `engine`
  depends on `core` only; only apps use `rfd`/`eframe`. The UI talks to a `QuoteSession`
  (local `Session` or `RemoteSession`).
- **Everything is a command** (`crates/engine/src/cmd.rs`): the UI, the CLI and the MCP server
  call the same commands; a new user-visible feature is a command first.
- **The UI is thin**: it reads `State` and acts through `QuoteApp::run`; colours from
  `theme::Tokens`, icons from `icons.rs` (drawn in code). Programmatic calls never open dialogs.
- No vendor artwork, ever.

## Working
- Quality gate before every commit: `cargo xtask ci` (fmt, clippy -D warnings, tests, layers,
  wasm). **Never break wasm**: `apps/quotecraft-web` must build for `wasm32-unknown-unknown`;
  use `web_time` instead of `std::time` in code the web app reaches.
- Interface strings are English in code and translated in `crates/ui-egui/src/i18n/` (add
  every new label to `i18n::LABELS` and the Italian table).
- For UI work, look at the result:
  `cargo run -p quotecraft-ui-egui --example snapshot -- out.png --lang it [--list|--items|--customers|--templates|--settings]`.
- Try it end to end: `cargo run -p quotecraft-cli -- quote show 1 --demo --lang it`.
- Update `ROADMAP.md` (progress log) and `docs/` when work lands.
