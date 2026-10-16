//! The demo data: a made-up price list (connectivity, hardware, services), two customers and
//! a quote. It is what `--demo` shows and what the tests run on.

use crate::config::Company;
use crate::model::*;
use crate::money::Cents;
use crate::store::Data;

fn item(code: &str, name: &str, description: &str, category: Category, unit: &str, one_off: (i64, i64), monthly: (i64, i64)) -> Item {
    Item {
        code: code.into(),
        name: name.into(),
        description: description.into(),
        category,
        unit: unit.into(),
        one_off: Prices { direct: Cents(one_off.0), wholesale: Cents(one_off.1) },
        monthly: Prices { direct: Cents(monthly.0), wholesale: Cents(monthly.1) },
        vat_pct: 22.0,
        active: true,
        tags: Vec::new(),
        updated: 1_760_000_000,
    }
}

pub fn company() -> Company {
    Company {
        name: "Esempio Telecomunicazioni S.r.l.".into(),
        address: "Via delle Reti 1".into(),
        cap: "09100".into(),
        city: "Cagliari".into(),
        province: "CA".into(),
        vat: "01234567890".into(),
        email: "commerciale@esempio.example".into(),
        pec: "esempio@pec.example".into(),
        phone: "070 000000".into(),
        web: "https://esempio.example".into(),
        footer: "Prezzi IVA esclusa. Pagamento: bonifico 30 gg data fattura. Offerta soggetta a verifica di copertura.".into(),
        ..Default::default()
    }
}

pub fn data() -> Data {
    let items = vec![
        item(
            "FTTH-1G",
            "FTTH 1 Gbps",
            "Fibra ottica fino a 1 Gbps in download / 300 Mbps in upload, IP statico incluso",
            Category::Connectivity,
            "mese",
            (9900, 4900),
            (3990, 2490),
        ),
        item("FTTH-2.5G", "FTTH 2,5 Gbps", "Fibra ottica XGS-PON fino a 2,5 Gbps", Category::Connectivity, "mese", (9900, 4900), (5990, 3990)),
        item("FTTC-200", "FTTC 200 Mbps", "Fibra misto rame fino a 200 Mbps", Category::Connectivity, "mese", (4900, 2900), (2990, 1990)),
        item(
            "FWA-100",
            "Radio FWA 100 Mbps",
            "Collegamento radio 5 GHz fino a 100 Mbps simmetrici, antenna inclusa in comodato",
            Category::Connectivity,
            "mese",
            (19900, 12000),
            (4990, 3490),
        ),
        item(
            "VOIP-LINEA",
            "Linea VoIP",
            "Numero geografico con 1 canale voce, traffico nazionale incluso",
            Category::Connectivity,
            "mese",
            (1000, 500),
            (490, 290),
        ),
        item(
            "RT-AX",
            "Router Wi-Fi 6",
            "Router dual band Wi-Fi 6, 4 porte gigabit, gestione remota",
            Category::Hardware,
            "pz",
            (14900, 9900),
            (0, 0),
        ),
        item("ONT-XGS", "ONT XGS-PON", "Terminale ottico 10 Gbps per FTTH 2,5 Gbps", Category::Hardware, "pz", (19900, 14900), (0, 0)),
        item("SW-24", "Switch 24 porte PoE", "Switch gestito 24 porte gigabit PoE+ con 4 SFP+", Category::Hardware, "pz", (49900, 39900), (0, 0)),
        item("AP-WIFI", "Access point Wi-Fi 6", "Access point da soffitto, alimentazione PoE", Category::Hardware, "pz", (12900, 8900), (0, 0)),
        item(
            "INST-BASE",
            "Installazione e configurazione",
            "Intervento tecnico in sede, fino a 2 ore",
            Category::Services,
            "pz",
            (12000, 8000),
            (0, 0),
        ),
        item("TECH-ORA", "Assistenza tecnica", "Tecnico in sede o da remoto, tariffa oraria", Category::Services, "ora", (6000, 4500), (0, 0)),
        item(
            "MON-24",
            "Monitoraggio 24/7",
            "Monitoraggio della linea e degli apparati con avviso guasti",
            Category::Services,
            "mese",
            (0, 0),
            (1500, 900),
        ),
        item("SLA-4H", "SLA ripristino 4 ore", "Intervento garantito entro 4 ore lavorative", Category::Services, "mese", (0, 0), (2900, 1900)),
    ];
    let customers = vec![
        Customer {
            id: 1,
            name: "Rossi & Figli <script>S.r.l.".into(),
            address: "Via Roma 10".into(),
            cap: "09124".into(),
            city: "Cagliari".into(),
            province: "CA".into(),
            vat: "02345678901".into(),
            email: "amministrazione@rossi.example".into(),
            pec: "rossi@pec.example".into(),
            sdi: "M5UXCR1".into(),
            contact: "Mario Rossi".into(),
            phone: "070 111111".into(),
            ..Default::default()
        },
        Customer {
            id: 2,
            name: "Partner Reti S.p.A.".into(),
            address: "Viale Marconi 100".into(),
            cap: "09131".into(),
            city: "Cagliari".into(),
            province: "CA".into(),
            vat: "03456789012".into(),
            email: "wholesale@partnerreti.example".into(),
            contact: "Anna Verdi".into(),
            ..Default::default()
        },
    ];
    let mut quote = Quote {
        id: 1,
        number: "2026-0001".into(),
        date: "2026-10-16".into(),
        channel: Channel::Direct,
        status: Status::Draft,
        customer: customers[0].clone(),
        title: "Connettività e rete per la sede di Via Roma".into(),
        validity_days: 30,
        duration_months: 24,
        author: "demo".into(),
        created: 1_760_600_000,
        updated: 1_760_600_000,
        ..Default::default()
    };
    for (code, qty) in [("FTTH-1G", 1.0), ("RT-AX", 1.0), ("AP-WIFI", 2.0), ("INST-BASE", 1.0), ("MON-24", 1.0)] {
        if let Some(i) = items.iter().find(|i| i.code == code) {
            quote.lines.push(Line::from_item(i, Channel::Direct, qty));
        }
    }
    if let Some(l) = quote.lines.iter_mut().find(|l| l.code == "AP-WIFI") {
        l.discount_pct = 10.0;
    }
    let mut counters = std::collections::BTreeMap::new();
    counters.insert("2026".to_string(), 1);
    Data { items, customers, quotes: vec![quote], counters, next_customer_id: 3, next_quote_id: 2 }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_demo_adds_up() {
        let d = data();
        assert_eq!(d.items.len(), 13);
        let t = d.quotes[0].totals();
        assert_eq!(t.one_off_net, Cents(9900 + 14900 + 23220 + 12000));
        assert_eq!(t.monthly_net, Cents(3990 + 1500));
        assert!(d.items.iter().all(|i| !i.code.is_empty() && i.active));
    }
}
