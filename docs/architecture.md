# Architecture

> **Last updated:** 2026-10-16

| Crate | Layer | What it holds |
|---|---|---|
| `crates/core` (`quotecraft-core`) | L0 | Money in cents (`money.rs`), dates without a calendar crate (`date.rs`), the price list, customers, quotes and their totals (`model.rs`), the JSON store (`store.rs`), the configuration and secrets (`config.rs`), the HTML of a quote and the placeholder fields (`render.rs`), the bridge traits (`bridge.rs`), the demo data (`demo.rs`). No network, no process: wasm-safe. |
| `crates/bridge` (`quotecraft-bridge`) | L1 | `WordCraft`: DOCX and PDF through `wordcraft-cli`; `MailCraft`: the mail through MailCraft's server API. |
| `crates/engine` (`quotecraft-engine`) | L5 | `Session`: the configuration, the shared store, the state, the user and their rights; the command table (`cmd.rs`); the worker that renders and sends (`worker.rs`); the snapshot a browser mirrors. |
| `crates/ui-egui` (`quotecraft-ui-egui`) | L6 | `QuoteApp`: the quotes and their editor, the price list, the customers, the letterheads, the settings; i18n, theme, icons drawn in code. The `snapshot` example renders it offscreen. |
| `crates/mcp` (`quotecraft-mcp`) | L6 | The MCP server (stdio): every tool is a command. |
| `crates/remote` (`quotecraft-remote`) | L6 | `RemoteSession`: a `QuoteSession` over `quotecraft-cli serve`'s API (native and wasm). |
| `apps/quotecraft` | L7 | The desktop app (eframe). |
| `apps/quotecraft-cli` | L7 | The CLI, `mcp`, and `serve` (the HTTP API, the web app's files, the documents' download). |
| `apps/quotecraft-web` | L7 | The web app (wasm32, trunk). |

## The data

One JSON file, `data.json`, holds the price list, the customers, the quotes and the counters;
it is written atomically after every change. A server shares one `Store` between every user's
session (`SharedStore`); each command locks it briefly. The configuration (`config.json`: the
company, the preferences) and the secrets (`secrets.json`: MailCraft's token) are files next to
it; letterheads live in `templates/` (or the folder the preference names), the documents produced
in `out/<quote>/`.

## A quote

`Quote` carries a copy of the customer and its lines (each a copy of the item at the time, with
the channel's price, quantity, discount and VAT), so a later change to the price list never
changes a quote already written. `Quote::totals` sums one-off and monthly amounts net and gross,
and the contract value over the duration. Numbers are `YYYY-NNNN`, one counter per year.

## Documents

`render::html` lays the quote out (the direct layout, or the wholesale "Allegato A" with the
signature block); `render::fields` gives the `{{placeholders}}` a letterhead may carry. A
`RenderJob` goes to the worker: the HTML is always written (`bridge::write_html`, filling an
HTML letterhead when there is one), then the `Renderer` adds the DOCX and PDF: WordCraft's
`convert` for the built-in layout and HTML letterheads, or `run --file letterhead.docx` with
`edit.replaceAll` for the placeholders, `edit.find {{corpo}}` + `insert.textFromFile` for the
body, `file.exportPdf` and `--save` for DOCX letterheads.

## Mail

`quote.send` collects the documents, the recipients (the customer's e-mail by default) and the
text (the preferences' subject and body with `{numero}`, `{cliente}`, `{azienda}` filled), and
the `Sender` posts `mail.send` to MailCraft's server with the service token and
`X-Mailcraft-User` naming the user, so the mail goes out from the user's own account; the job
is awaited through MailCraft's snapshot. The quote becomes `sent`.

## Commands

| Command | Who | Answer |
|---|---|---|
| `items.list`, `items.get` | everyone | the price list |
| `items.save`, `items.remove`, `items.import` | administrators | the item |
| `customers.list`, `customers.save`, `customers.remove` | everyone | |
| `quotes.list`, `quote.new`, `quote.open`, `quote.get`, `quote.set`, `quote.delete`, `quote.duplicate` | everyone | the quote with totals |
| `quote.addLine`, `quote.setLine`, `quote.removeLine`, `quote.moveLine` | everyone | the quote |
| `quote.html` | everyone | HTML and fields |
| `quote.render`, `quote.send`, `bridge.check` | everyone | `{job}` then the files / the send |
| `templates.list` / `templates.import`, `templates.remove` | everyone / administrators | |
| `company.get` / `company.set`, `prefs.get` / `prefs.set` | everyone / administrators (language and theme: everyone) | |
| `session.user`, `state.get` | everyone | |

## Revision history

| Date | Change |
|---|---|
| 2026-10-16 | First version. |
