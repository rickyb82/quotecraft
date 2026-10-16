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
- A WordCraft table layout for the lines when the HTML import loses borders.

## Progress log
- 2026-10-16: first version: price lists, customers, quotes (direct and Allegato A), HTML +
  WordCraft documents, MailCraft sending, UI, CLI, MCP, server, Docker.
