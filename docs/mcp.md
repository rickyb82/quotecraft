# MCP server

> **Last updated:** 2026-10-16

QuoteCraft speaks the [Model Context Protocol](https://modelcontextprotocol.io) so an assistant
can read the price list, write quotes, make the documents and send them: `quotecraft-cli mcp`
serves MCP over stdio (newline-delimited JSON-RPC 2.0, protocol `2025-06-18`; `2025-03-26` and
`2024-11-05` are accepted). Every tool is an engine command.

```sh
claude mcp add quotecraft -- quotecraft-cli mcp                       # the local data
claude mcp add quotecraft -- quotecraft-cli mcp --server https://quotecraft.example --token T --user anna   # a server
```

`--demo` serves the built-in data. Against a server, the user and the role come from the
framework: an assistant working for a non-administrator cannot change the price list.

## Tools

| Tool | What it does |
|---|---|
| `list_items`, `get_item` | the price list (filter by text and category) |
| `save_item`, `remove_item`, `import_items` | change the price list (administrators) |
| `list_customers`, `save_customer`, `remove_customer` | the customers |
| `list_quotes`, `new_quote`, `get_quote`, `set_quote`, `delete_quote`, `duplicate_quote` | the quotes |
| `add_line`, `set_line`, `remove_line`, `move_line` | the lines (from the price list by code, or custom) |
| `quote_html` | the quote as HTML, the placeholder fields and the tables |
| `make_documents` | the HTML, and the PDF and DOCX through WordCraft |
| `attach_document`, `remove_document` | a document made elsewhere (base64 or a server path) on the quote; remove one |
| `send_quote` | the mail through MailCraft, from the user's account (the tool says to ask first); the formats, or the documents by name with `files` |
| `list_letterheads`, `import_letterhead`, `remove_letterhead` | the letterheads |
| `get_company`, `set_company`, `get_preferences`, `set_preferences`, `check_bridges` | settings |

Resources: `quotecraft://quote` (the open quote as HTML), `quotecraft://pricelist`. Prompt:
`write_quote` (a quote from a request in plain words).

## Revision history

| Date | Change |
|---|---|
| 2026-10-16 | First version. |
| 2026-10-16 | `attach_document`, `remove_document`; `send_quote` takes `files`; `list_letterheads` lists tables and sections too. |
