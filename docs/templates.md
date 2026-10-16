# Letterheads and placeholders

> **Last updated:** 2026-10-16

A letterhead is the company's paper: a `.docx` (header, footer, logo, fonts, the general
conditions of the Allegato A…) or an `.html` file. QuoteCraft fills it with the quote and
WordCraft turns it into the DOCX and PDF sent to the customer. Without a letterhead, the
built-in layout is used (company header from Settings, then the quote).

Put the file in the letterheads folder (`/data/templates`, or the folder the preference
`templates_dir` names), pick it in the quote's header, and set the defaults per channel in
Settings → Letterheads. In the app, Letterheads → "Add a letterhead": choose the file with the
dialog or drop it on the window; it is uploaded to the folder (through the server on the web,
so the browser needs no access to the server's disk). The path field remains for files
already on the server.

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

## Tables: the lines where you want them

When the company's paper has its own structure (a table of fees here, the activations there,
the general conditions in between), place these instead of `{{corpo}}`. Each becomes a table
laid out by QuoteCraft, in a `.docx` as well as in HTML:

| Placeholder | Table |
|---|---|
| `{{tabella.righe}}` | all the lines: code, description, quantity, one-off and monthly prices, discount, totals |
| `{{tabella.una_tantum}}` | the lines with a one-off price: quantity, price, discount, total, with a total row |
| `{{tabella.canone}}` | the lines with a monthly fee: quantity, fee, discount, total, with a total row |
| `{{tabella.connettivita}}`, `{{tabella.hardware}}`, `{{tabella.servizi}}` | the lines of one category |
| `{{tabella.riepilogo}}` | the totals: net, VAT, gross for one-off, monthly and the contract value |

In a `.docx`, put each one in a paragraph of its own: the paragraph is replaced by the table.
A letterhead with tables (or sections) and no `{{corpo}}` gets no body appended: it lays the
lines out itself. With `{{corpo}}` too, both are filled.

## Sections: one row per line (HTML letterheads)

An `.html` letterhead can repeat a piece of itself for every line, so the rows take the
letterhead's own style. The text between `{{#righe}}` and `{{/righe}}` is written once per
line, with the line's fields filled in:

| Section | Lines |
|---|---|
| `{{#righe}}…{{/righe}}` | every line |
| `{{#righe.una_tantum}}…{{/righe.una_tantum}}` | the lines with a one-off price |
| `{{#righe.canone}}…{{/righe.canone}}` | the lines with a monthly fee |
| `{{#righe.connettivita}}…`, `{{#righe.hardware}}…`, `{{#righe.servizi}}…` | the lines of one category |

| Field of a line | Meaning |
|---|---|
| `{{riga.n}}` | position (1, 2…) |
| `{{riga.codice}}`, `{{riga.nome}}`, `{{riga.descrizione}}`, `{{riga.categoria}}` | the item |
| `{{riga.qta}}`, `{{riga.unita}}` | quantity and unit |
| `{{riga.una_tantum}}`, `{{riga.canone}}` | unit prices (empty when zero) |
| `{{riga.sconto}}` | discount (`10%`, or empty) |
| `{{riga.iva}}` | VAT rate |
| `{{riga.totale_una_tantum}}`, `{{riga.totale_canone}}` | the line's totals (quantity and discount applied) |

```html
<table border="1">
<tr><th>Servizio</th><th>Q.tà</th><th>Canone mensile</th><th>Totale</th></tr>
{{#righe.canone}}<tr><td>{{riga.nome}}</td><td>{{riga.qta}}</td><td>{{riga.canone}}</td><td>{{riga.totale_canone}}</td></tr>{{/righe.canone}}
</table>
{{tabella.riepilogo}}
```

Sections are not available in a `.docx` (Word has no way to mark a repeating row): use the
tables there.

## Documents made elsewhere

A quote's folder holds the files QuoteCraft makes (`quote.render`) and any file attached to it
(`quote.attach`: a revised PDF, a drawing, a spreadsheet; up to 50 MB, with a document, image
or text extension). In the app: Documents → "Attach a file…", or drop the file on the window
while the quote is open; from an assistant: `attach_document`. Attached files are listed with
the others, downloaded the same way and mailed by `quote.send` (by format, or by name with
`files`). `quote.detach` / "Remove from the quote" removes a document, made or attached.

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
| 2026-10-16 | Tables (`{{tabella.…}}`) and HTML sections (`{{#righe}}…{{/righe}}`) for letterheads that lay the lines out themselves; documents made elsewhere attached to the quote. |
