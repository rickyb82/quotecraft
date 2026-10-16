# Letterheads and placeholders

> **Last updated:** 2026-10-16

A letterhead is the company's paper: a `.docx` (header, footer, logo, fonts, the general
conditions of the Allegato A…) or an `.html` file. QuoteCraft fills it with the quote and
WordCraft turns it into the DOCX and PDF sent to the customer. Without a letterhead, the
built-in layout is used (company header from Settings, then the quote).

Put the file in the letterheads folder (`/data/templates`, or the folder the preference
`templates_dir` names), pick it in the quote's header, and set the defaults per channel in
Settings → Letterheads.

## Placeholders

Write them as they are, anywhere in the document (body, header, footer, tables):

| Placeholder | Meaning |
|---|---|
| `{{corpo}}` | where the quote's body goes (the lines by category, the totals, the notes, the Allegato A signature block); without it the body goes at the end of the document |
| `{{numero}}` | quote number (`2026-0001`) |
| `{{data}}` | date (`GG/MM/AAAA`) |
| `{{scadenza}}` | expiry date |
| `{{validita_giorni}}` | validity in days |
| `{{durata_mesi}}` | contract duration in months |
| `{{titolo}}` | title |
| `{{canale}}` | channel (Diretta / Wholesale) |
| `{{note}}` | notes |
| `{{autore}}` | who wrote it |
| `{{cliente.nome}}`, `{{cliente.indirizzo}}`, `{{cliente.cap}}`, `{{cliente.citta}}`, `{{cliente.provincia}}`, `{{cliente.indirizzo_completo}}` | the customer |
| `{{cliente.piva}}`, `{{cliente.cf}}`, `{{cliente.sdi}}`, `{{cliente.email}}`, `{{cliente.pec}}`, `{{cliente.referente}}`, `{{cliente.telefono}}` | the customer's codes and contacts |
| `{{azienda.nome}}`, `{{azienda.indirizzo}}`, `{{azienda.piva}}`, `{{azienda.email}}`, `{{azienda.pec}}`, `{{azienda.telefono}}`, `{{azienda.web}}`, `{{azienda.iban}}` | your company (Settings) |
| `{{totale.una_tantum}}`, `{{totale.una_tantum_iva}}`, `{{totale.una_tantum_lordo}}` | one-off totals |
| `{{totale.canone}}`, `{{totale.canone_iva}}`, `{{totale.canone_lordo}}` | monthly totals |
| `{{totale.contratto}}`, `{{totale.contratto_lordo}}` | contract value (one-off + monthly × duration) |
| `{{righe}}` | number of lines |

In a `.docx`, type each placeholder in one go (Word splits text edited in pieces into several
runs, and the placeholder must be found as a whole: retype it if it is not replaced).

## An HTML letterhead

```html
<!doctype html>
<html lang="it"><head><meta charset="utf-8"><style>body{font-family:Helvetica,Arial;font-size:10.5pt}</style></head>
<body>
<p><b>{{azienda.nome}}</b><br>{{azienda.indirizzo}}<br>P. IVA {{azienda.piva}}</p>
<p>Spett.le {{cliente.nome}}<br>{{cliente.indirizzo_completo}}</p>
{{corpo}}
<p>Firma per accettazione: ____________________</p>
</body></html>
```

## Revision history

| Date | Change |
|---|---|
| 2026-10-16 | First version. |
