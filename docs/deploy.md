# Deploying QuoteCraft in a framework (Emily)

> **Last updated:** 2026-10-16

QuoteCraft's server (`quotecraft-cli serve`) hosts the web app and an HTTP API for a framework
that puts it behind its own login and HTTPS proxy. The documents come from WordCraft's CLI in
the same container; the mail goes through MailCraft's server.

## Docker

```sh
QUOTECRAFT_TOKEN=$(openssl rand -hex 24) docker compose up -d
```

The container listens on `127.0.0.1:7878`, serves the web app at `/` and the API under `/api/`.
Put the framework's reverse proxy in front (HTTPS), forwarding the user's name in
`X-Quotecraft-User`, the service token in `Authorization: Bearer …`, and **`X-Quotecraft-Role:
admin` for the users who may change the price list, the letterheads, the company and the
settings**; everyone else reads the price list and writes quotes. The page passes `?token=…
&user=…` on and remembers them in the browser; `?settings=off` hides the settings screen and its
button for non-administrators (the server refuses the commands anyway).

The image builds `wordcraft-cli` from `rickyb82/wordcraft` (`WORDCRAFT_REPO`, `WORDCRAFT_REV`
build arguments): a long first build, cached afterwards.

## MailCraft

Sending uses MailCraft's server as the user: set its URL and the framework's MailCraft service
token once (an administrator):

```sh
docker exec quotecraft quotecraft-cli prefs mailcraft_url=http://mailcraft:7878 mailcraft_token=THE_MAILCRAFT_SERVICE_TOKEN
```

The token goes to `/data/secrets.json` (owner-only) and is never returned by a command. The
user must have a mail account in MailCraft: the quote goes out from it, with the documents
attached. "Check WordCraft and MailCraft" in Settings (or `quotecraft-cli check`) says whether
both are reachable.

## Letterheads

Letterheads are `.docx` or `.html` files in `/data/templates` (or the folder the preference
`templates_dir` names: mount the framework's shared "Commerciale" folder there). An
administrator imports one from the Settings → Letterheads screen (a path on the server), with
`templates.import` over the API or MCP (a path, or the file as base64), or by copying the file
into the folder. See `docs/templates.md` for the placeholders.

## The API

| | |
|---|---|
| `POST /api/cmd` `{"command", "params"}` | runs a command; `{"result"}` or `{"error"}`; a job answers `{"result": {"job": N}}` |
| `GET /api/snapshot?since=N` | the state when it changed (config, user, admin, state, completed jobs) |
| `GET /api/files/{quote}/{name}` | a document produced (`quote` is the quote's file base, e.g. `2026-0001-rossi-srl`) |
| `GET /api/health` | `{"ok": true}` |

Headers: `Authorization: Bearer TOKEN`, `X-Quotecraft-User: name`, `X-Quotecraft-Role: admin`
(or `?token=`, `?user=`, `?role=` for the files and the page).

## Without a framework

`quotecraft-cli serve --everyone-admin --open` on a workstation: every user is an administrator;
or the desktop app, where the owner is.

## Revision history

| Date | Change |
|---|---|
| 2026-10-16 | First version. |
