# QuoteCraft

**Preventivatore con listino prezzi per il reparto commerciale**, in Rust, della famiglia *craft*.
Tiene i listini (connettività, fornitura hardware, fornitura servizi, con prezzo diretta e
wholesale, una tantum e canone), scrive i preventivi per la diretta e l'**Allegato A** per il
wholesale, produce i documenti con **WordCraft** (DOCX e PDF, anche sulla vostra carta intestata)
e li spedisce con **MailCraft** dall'account dell'utente. Interfaccia egui (desktop e web),
CLI e server MCP: tutto quello che fa l'interfaccia lo fa anche un assistente.

## Come funziona

1. **Listino**: ogni voce ha codice, nome, categoria, unità, prezzo una tantum e canone mensile
   per la diretta e per il wholesale, IVA. Lo modificano solo gli amministratori del framework.
2. **Preventivo**: canale (diretta o wholesale), cliente (dall'anagrafica o scritto al momento),
   voci dal listino con quantità e sconto, voci libere; totali una tantum, canone, valore del
   contratto, IVA; validità e durata. Numerazione `AAAA-NNNN`.
3. **Documenti**: "Genera i documenti" scrive l'HTML e, tramite `wordcraft-cli`, il PDF e il DOCX
   sulla carta intestata scelta (`.docx` o `.html` con segnaposto come `{{cliente.nome}}` e
   `{{corpo}}`, vedi `docs/templates.md`).
4. **Mail**: "Invia via mail" passa i documenti a MailCraft, che li spedisce dall'account di chi
   sta lavorando; il preventivo passa a *Inviato*.

## Provare

```sh
cargo run -p quotecraft -- --demo --lang it                       # desktop, dati di esempio
cargo run -p quotecraft-cli -- quote show 1 --demo --lang it      # CLI
cargo run -p quotecraft-cli -- quote new --channel wholesale --customer 2 --demo
cargo run -p quotecraft-cli -- quote add 2 FTTH-1G 3 --discount 10 --demo
cargo run -p quotecraft-cli -- mcp --demo                         # server MCP (stdio)
```

Senza `--demo` i dati vivono in `~/.config/quotecraft` (o `QUOTECRAFT_HOME`): `data.json`,
`config.json`, `secrets.json`, `templates/`, `out/`.

## In un framework (Emily)

`docker compose up -d` avvia il server con la web app e `wordcraft-cli`; il framework passa
utente e ruolo nelle intestazioni (`X-Quotecraft-User`, `X-Quotecraft-Role: admin`) e il token di
servizio. MailCraft va indicato una volta nelle preferenze. Dettagli in `docs/deploy.md`.

## Architettura

`core` (modello, denaro, HTML) → `bridge` (WordCraft, MailCraft) → `engine` (sessione, comandi,
operazioni) → `ui-egui` / `mcp` / `remote` → `apps/*`. Dettagli in `docs/architecture.md`.
Gate di qualità: `cargo xtask ci`.

## Licenza

MIT o Apache-2.0, a scelta.
