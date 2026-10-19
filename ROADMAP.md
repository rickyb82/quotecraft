# QuoteCraft roadmap

> **Last updated:** 2026-10-16 · **Stage:** first version, in use by the sales team through Emily

QuoteCraft keeps the price lists and writes the quotes; WordCraft makes the documents and
MailCraft sends them. Detail in `docs/`: `architecture.md`, `deploy.md`, `templates.md`, `mcp.md`.

## Done
- Price list: connectivity, hardware, services; one-off and monthly prices, direct and wholesale;
  VAT, units, tags; administrators only.
- Customers; quotes for direct customers and the wholesale Allegato A: lines from the price list
  or custom, discounts, totals (one-off, monthly, contract value, VAT), numbering per year.
- Documents: the quote's HTML; DOCX and PDF through `wordcraft-cli`, on a `.docx` or `.html`
  letterhead with placeholders; download from the web app.
- Mail through MailCraft from the user's own account, documents attached.
- egui interface (desktop and web), CLI, MCP server, server for a framework (users and roles
  from headers), Docker image with WordCraft's CLI.

## Next
- Import the price list from CSV/XLSX (the CLI takes JSON today).
- Quote versions (a revision history per quote) and expiry marked automatically.
- Margins: an optional cost per item and the margin of a quote.

## Progress log
- 2026-10-19: a `.docx` letterhead keeps its own tables: a row with `{{riga.*}}` fields is written once per line before WordCraft runs (`bridge::docx`), with its shading, borders and fonts; `{{#righe.una_tantum}}`…`{{/righe.una_tantum}}` in the row picks the lines (one row or several, never across tables), an empty section removes its rows, a placeholder Word split across runs is joined. Checked with WordCraft on a company letterhead.
- 2026-10-16: letterheads lay the lines out themselves: `{{tabella.canone}}`, `{{tabella.una_tantum}}`, `{{tabella.riepilogo}}`… (DOCX and HTML, found in the DOCX with `wordcraft-cli text`) and `{{#righe}}…{{/righe}}` sections with `{{riga.*}}` fields (HTML); a document made elsewhere joins the quote (`quote.attach`/`quote.detach`, "Attach a file…" or drop, MCP `attach_document`), and `quote.send` can name the files.
- 2026-10-16: letterheads can be chosen with a file dialog or dropped on the window, on the desktop and in the browser (uploaded through `templates.import` with the file's bytes); the server path stays as an option.
- 2026-10-16: checked end to end with wordcraft-cli 0.6: built-in layout and HTML/DOCX letterheads to DOCX and PDF; the tables carry borders WordCraft keeps.
- 2026-10-16: first version: price lists, customers, quotes (direct and Allegato A), HTML +
  WordCraft documents, MailCraft sending, UI, CLI, MCP, server, Docker.
