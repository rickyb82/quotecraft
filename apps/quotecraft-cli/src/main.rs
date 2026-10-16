//! QuoteCraft headless: the CLI, the MCP server and the web app's server.
#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::unimplemented, clippy::todo, clippy::unreachable)]

mod serve;

use std::process::ExitCode;

use quotecraft_engine::Session;
use quotecraft_engine::core::config::Paths;
use serde_json::{Value, json};

const USAGE: &str = "QuoteCraft: price lists and quotes for the sales team.

Usage:
  quotecraft-cli items [QUERY] [--category connectivity|hardware|services] [--all]
  quotecraft-cli item set CODE key=value ...        # name=, category=, unit=, one_off_direct=, monthly_wholesale=, vat_pct=, active=
  quotecraft-cli item remove CODE
  quotecraft-cli items import FILE.json [--replace] # a JSON list of items
  quotecraft-cli customers [QUERY]
  quotecraft-cli quotes [--mine] [--status draft|sent|accepted|rejected|expired]
  quotecraft-cli quote new --channel direct|wholesale [--customer ID] [--title T]
  quotecraft-cli quote show ID [--html]
  quotecraft-cli quote add ID CODE [QTY] [--discount PCT]
  quotecraft-cli quote set ID key=value ...          # title=, status=, template=, validity_days=, duration_months=
  quotecraft-cli quote render ID [--formats pdf,docx,html]
  quotecraft-cli quote send ID [--to A@B] [--cc C@D] [--subject S] [--body B]
  quotecraft-cli templates [--import FILE [--name N]]
  quotecraft-cli company [key=value ...]
  quotecraft-cli prefs [key=value ...]
  quotecraft-cli check                               # WordCraft and MailCraft
  quotecraft-cli run --cmd 'id={\"json\"}' [--cmd ...]
  quotecraft-cli mcp
  quotecraft-cli mcp --server URL [--token T] [--user NAME] [--role admin] [--mail-user ID]
  quotecraft-cli serve [--bind 127.0.0.1:7878] [--static DIR] [--token SECRET] [--everyone-admin] [--open]
  quotecraft-cli commands

Options: --demo (the built-in data), --home DIR (data directory), --lang en|it, --user NAME, --json
Environment: QUOTECRAFT_HOME, QUOTECRAFT_TOKEN, QUOTECRAFT_LANG";

fn flag(args: &[String], name: &str) -> Option<String> {
    args.iter().position(|a| a == name).and_then(|i| args.get(i + 1).cloned())
}

fn has(args: &[String], name: &str) -> bool {
    args.iter().any(|a| a == name)
}

/// Positional arguments (not flags nor their values).
fn positional(args: &[String]) -> Vec<String> {
    let valued = [
        "--home",
        "--lang",
        "--user",
        "--role",
        "--mail-user",
        "--category",
        "--status",
        "--channel",
        "--customer",
        "--title",
        "--discount",
        "--formats",
        "--to",
        "--cc",
        "--subject",
        "--body",
        "--import",
        "--name",
        "--cmd",
        "--server",
        "--token",
        "--bind",
        "--static",
    ];
    let mut out = Vec::new();
    let mut skip = false;
    for a in args {
        if skip {
            skip = false;
            continue;
        }
        if valued.contains(&a.as_str()) {
            skip = true;
            continue;
        }
        if a.starts_with("--") {
            continue;
        }
        out.push(a.clone());
    }
    out
}

fn paths(args: &[String]) -> Paths {
    match flag(args, "--home") {
        Some(h) => Paths::under(std::path::Path::new(&h)),
        None => Paths::default_dirs(),
    }
}

fn session(args: &[String]) -> Result<Session, String> {
    let mut s = if has(args, "--demo") {
        Session::demo_with(Some(quotecraft_bridge::renderer_factory()), Some(quotecraft_bridge::sender_factory()))
    } else {
        let p = paths(args);
        let store = Session::open_store(&p).map_err(|e| e.to_string())?;
        Session::new(store, Some(quotecraft_bridge::renderer_factory()), Some(quotecraft_bridge::sender_factory()), Some(p))
    };
    if let Some(l) = flag(args, "--lang").or_else(|| std::env::var("QUOTECRAFT_LANG").ok()) {
        let _ = s.execute("prefs.set", &json!({"language": l}));
    }
    let user = flag(args, "--user").or_else(|| std::env::var("USER").ok()).or_else(|| std::env::var("USERNAME").ok()).unwrap_or_default();
    s.set_user(&user, true);
    Ok(s)
}

fn print_json(v: &Value) {
    println!("{}", serde_json::to_string_pretty(v).unwrap_or_default());
}

/// `key=value` pairs from the positional arguments after `skip`.
fn pairs(pos: &[String], skip: usize) -> Value {
    let mut params = json!({});
    for kv in pos.iter().skip(skip) {
        if let Some((k, v)) = kv.split_once('=') {
            params[k] = serde_json::from_str(v).unwrap_or_else(|_| json!(v));
        }
    }
    params
}

fn run(args: &[String]) -> Result<(), String> {
    let pos = positional(args);
    let json_out = has(args, "--json");
    let e = |e: quotecraft_engine::Error| e.to_string();
    match pos.first().map(String::as_str) {
        None | Some("--help") | Some("-h") | Some("help") => {
            println!("{USAGE}");
            Ok(())
        }
        Some("commands") => {
            for c in quotecraft_engine::COMMANDS {
                println!("{:<20} {:<36} {}{}", c.id, c.label, if c.admin { "[admin] " } else { "" }, c.params);
            }
            Ok(())
        }
        Some("items") if pos.get(1).map(String::as_str) == Some("import") => {
            let file = pos.get(2).ok_or("items import: give the JSON file")?;
            let text = std::fs::read_to_string(file).map_err(|e| format!("{file}: {e}"))?;
            let rows: Value = serde_json::from_str(&text).map_err(|e| format!("{file}: {e}"))?;
            let rows = rows.get("items").cloned().unwrap_or(rows);
            let mut s = session(args)?;
            let v = s.run_blocking("items.import", json!({"rows": rows, "replace": has(args, "--replace")})).map_err(e)?;
            print_json(&v);
            Ok(())
        }
        Some("items") => {
            let mut s = session(args)?;
            let mut p = json!({"query": pos.get(1).cloned().unwrap_or_default(), "all": has(args, "--all")});
            if let Some(c) = flag(args, "--category") {
                p["category"] = json!(c);
            }
            let v = s.run_blocking("items.list", p).map_err(e)?;
            if json_out {
                print_json(&v);
            } else {
                for i in v["items"].as_array().into_iter().flatten() {
                    println!(
                        "{:<12} {:<34} {:<13} {:>6} {:>10}/{:<10} {:>10}/{:<10}",
                        i["code"].as_str().unwrap_or_default(),
                        i["name"].as_str().unwrap_or_default(),
                        i["category"].as_str().unwrap_or_default(),
                        i["unit"].as_str().unwrap_or_default(),
                        cents(&i["one_off"]["direct"]),
                        cents(&i["one_off"]["wholesale"]),
                        cents(&i["monthly"]["direct"]),
                        cents(&i["monthly"]["wholesale"]),
                    );
                }
            }
            Ok(())
        }
        Some("item") => {
            let mut s = session(args)?;
            match pos.get(1).map(String::as_str) {
                Some("set") => {
                    let code = pos.get(2).ok_or("item set: give the code")?;
                    let mut p = pairs(&pos, 3);
                    p["code"] = json!(code);
                    print_json(&s.run_blocking("items.save", p).map_err(e)?);
                }
                Some("remove") => {
                    let code = pos.get(2).ok_or("item remove: give the code")?;
                    print_json(&s.run_blocking("items.remove", json!({"code": code})).map_err(e)?);
                }
                _ => return Err(USAGE.to_string()),
            }
            Ok(())
        }
        Some("customers") => {
            let mut s = session(args)?;
            let v = s.run_blocking("customers.list", json!({"query": pos.get(1).cloned().unwrap_or_default()})).map_err(e)?;
            if json_out {
                print_json(&v);
            } else {
                for c in v["customers"].as_array().into_iter().flatten() {
                    println!(
                        "{:>5} {:<40} {:<14} {}",
                        c["id"],
                        c["name"].as_str().unwrap_or_default(),
                        c["vat"].as_str().unwrap_or_default(),
                        c["email"].as_str().unwrap_or_default()
                    );
                }
            }
            Ok(())
        }
        Some("quotes") => {
            let mut s = session(args)?;
            let mut p = json!({"mine": has(args, "--mine")});
            if let Some(st) = flag(args, "--status") {
                p["status"] = json!(st);
            }
            let v = s.run_blocking("quotes.list", p).map_err(e)?;
            if json_out {
                print_json(&v);
            } else {
                for q in v["quotes"].as_array().into_iter().flatten() {
                    println!(
                        "{:>4} {:<10} {:<10} {:<30} {:<10} {:<9} {:>12} {:>12}/mese  {}",
                        q["id"],
                        q["number"].as_str().unwrap_or_default(),
                        q["date"].as_str().unwrap_or_default(),
                        q["customer"].as_str().unwrap_or_default().chars().take(30).collect::<String>(),
                        q["channel"].as_str().unwrap_or_default(),
                        q["status"].as_str().unwrap_or_default(),
                        cents(&q["one_off_net"]),
                        cents(&q["monthly_net"]),
                        q["author"].as_str().unwrap_or_default(),
                    );
                }
            }
            Ok(())
        }
        Some("quote") => {
            let mut s = session(args)?;
            let italian = s.italian();
            let id = |n: usize| -> Result<u64, String> { pos.get(n).and_then(|t| t.parse().ok()).ok_or_else(|| "give the quote id".to_string()) };
            let v = match pos.get(1).map(String::as_str) {
                Some("new") => {
                    let mut p = json!({"channel": flag(args, "--channel").unwrap_or_else(|| "direct".into()), "title": flag(args, "--title").unwrap_or_default()});
                    if let Some(c) = flag(args, "--customer") {
                        p["customer_id"] = json!(c);
                    }
                    s.run_blocking("quote.new", p).map_err(e)?
                }
                Some("show") => {
                    let id = id(2)?;
                    if has(args, "--html") {
                        let v = s.run_blocking("quote.html", json!({"id": id})).map_err(e)?;
                        println!("{}", v["html"].as_str().unwrap_or_default());
                        return Ok(());
                    }
                    s.run_blocking("quote.open", json!({"id": id})).map_err(e)?
                }
                Some("add") => {
                    let id = id(2)?;
                    let code = pos.get(3).ok_or("quote add: give the item code")?;
                    let mut p = json!({"id": id, "code": code, "qty": pos.get(4).cloned().unwrap_or_else(|| "1".into())});
                    if let Some(d) = flag(args, "--discount") {
                        p["discount_pct"] = json!(d);
                    }
                    s.run_blocking("quote.addLine", p).map_err(e)?
                }
                Some("set") => {
                    let id = id(2)?;
                    let mut p = pairs(&pos, 3);
                    p["id"] = json!(id);
                    s.run_blocking("quote.set", p).map_err(e)?
                }
                Some("render") => {
                    let id = id(2)?;
                    let formats: Vec<String> =
                        flag(args, "--formats").unwrap_or_else(|| "pdf,docx".into()).split(',').map(|f| f.trim().to_string()).collect();
                    s.run_blocking("quote.render", json!({"id": id, "formats": formats})).map_err(e)?
                }
                Some("send") => {
                    let id = id(2)?;
                    let mut p = json!({"id": id});
                    for (k, f) in [("to", "--to"), ("cc", "--cc"), ("subject", "--subject"), ("body", "--body"), ("formats", "--formats")] {
                        if let Some(v) = flag(args, f) {
                            p[k] = json!(v);
                        }
                    }
                    s.run_blocking("quote.send", p).map_err(e)?
                }
                _ => return Err(USAGE.to_string()),
            };
            if json_out || v.get("summary").is_none() {
                print_json(&v);
            } else {
                println!("{}", v["summary"].as_str().unwrap_or_default());
                for l in v["lines"].as_array().into_iter().flatten() {
                    println!(
                        "  {:<12} {:<34} {:>6} {:>12} {:>12}/mese",
                        l["code"].as_str().unwrap_or_default(),
                        l["name"].as_str().unwrap_or_default(),
                        l["qty"],
                        cents(&l["one_off"]),
                        cents(&l["monthly"])
                    );
                }
                let t = &v["totals"];
                println!(
                    "  {} {} + {}/mese ({} con IVA + {}/mese)",
                    if italian { "Totale:" } else { "Total:" },
                    cents(&t["one_off_net"]),
                    cents(&t["monthly_net"]),
                    cents(&t["one_off_gross"]),
                    cents(&t["monthly_gross"])
                );
            }
            Ok(())
        }
        Some("templates") => {
            let mut s = session(args)?;
            if let Some(file) = flag(args, "--import") {
                print_json(&s.run_blocking("templates.import", json!({"path": file, "name": flag(args, "--name").unwrap_or_default()})).map_err(e)?);
            }
            let v = s.run_blocking("templates.list", json!({})).map_err(e)?;
            if json_out {
                print_json(&v);
            } else {
                println!("{}", v["dir"].as_str().unwrap_or_default());
                for t in v["templates"].as_array().into_iter().flatten() {
                    println!(
                        "  {:<40} {:<5} {} kB",
                        t["name"].as_str().unwrap_or_default(),
                        t["kind"].as_str().unwrap_or_default(),
                        t["bytes"].as_u64().unwrap_or(0) / 1024
                    );
                }
            }
            Ok(())
        }
        Some("company") => {
            let mut s = session(args)?;
            let p = pairs(&pos, 1);
            let v =
                if p.as_object().is_some_and(|o| o.is_empty()) { s.run_blocking("company.get", json!({})) } else { s.run_blocking("company.set", p) };
            print_json(&v.map_err(e)?);
            Ok(())
        }
        Some("prefs") => {
            let mut s = session(args)?;
            let p = pairs(&pos, 1);
            let v = if p.as_object().is_some_and(|o| o.is_empty()) { s.run_blocking("prefs.get", json!({})) } else { s.run_blocking("prefs.set", p) };
            print_json(&v.map_err(e)?);
            Ok(())
        }
        Some("check") => {
            let mut s = session(args)?;
            print_json(&s.run_blocking("bridge.check", json!({})).map_err(e)?);
            Ok(())
        }
        Some("run") => {
            let mut s = session(args)?;
            let mut i = 0;
            while let Some(p) = args.iter().skip(i).position(|a| a == "--cmd") {
                let spec = args.get(i + p + 1).ok_or("--cmd needs a value")?;
                i += p + 2;
                let (id, params) = match spec.split_once('=') {
                    Some((id, raw)) => (id.to_string(), serde_json::from_str::<Value>(raw).map_err(|e| format!("{id}: {e}"))?),
                    None => (spec.clone(), json!({})),
                };
                match s.run_blocking(&id, params) {
                    Ok(v) => print_json(&json!({"command": id, "result": v})),
                    Err(e) => print_json(&json!({"command": id, "error": e.to_string()})),
                }
            }
            Ok(())
        }
        Some("mcp") => {
            let mut server = match flag(args, "--server") {
                Some(url) => quotecraft_mcp::Server::new(
                    quotecraft_remote::RemoteSession::new(&url, &flag(args, "--token").unwrap_or_default(), &flag(args, "--user").unwrap_or_default())
                        .with_identity(&flag(args, "--role").unwrap_or_default(), &flag(args, "--mail-user").unwrap_or_default()),
                ),
                None => quotecraft_mcp::Server::new(session(args)?),
            };
            let stdin = std::io::stdin();
            let stdout = std::io::stdout();
            server.serve(stdin.lock(), stdout.lock()).map_err(|e| e.to_string())
        }
        Some("serve") => {
            let static_dir = flag(args, "--static").map(std::path::PathBuf::from).or_else(|| {
                let candidates = [std::path::PathBuf::from("dist/web"), std::path::PathBuf::from("/app/web")];
                candidates.into_iter().find(|p| p.join("index.html").is_file())
            });
            serve::run(
                Some(quotecraft_bridge::renderer_factory()),
                Some(quotecraft_bridge::sender_factory()),
                serve::Options {
                    bind: flag(args, "--bind").unwrap_or_else(|| "127.0.0.1:7878".into()),
                    token: flag(args, "--token").or_else(|| std::env::var("QUOTECRAFT_TOKEN").ok()).unwrap_or_default(),
                    static_dir,
                    open: has(args, "--open"),
                    paths: Some(paths(args)),
                    demo: has(args, "--demo"),
                    lang: flag(args, "--lang").or_else(|| std::env::var("QUOTECRAFT_LANG").ok()),
                    everyone_admin: has(args, "--everyone-admin"),
                },
            )
        }
        Some(other) => Err(format!("unknown command `{}`\n{USAGE}", other.chars().take(40).collect::<String>())),
    }
}

/// Cents in a JSON number as `1.234,56`.
fn cents(v: &Value) -> String {
    quotecraft_engine::core::money::Cents(v.as_i64().unwrap_or(0)).fmt()
}

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match run(&args) {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("{e}");
            ExitCode::FAILURE
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn positional_skips_flag_values() {
        let args: Vec<String> = ["quote", "add", "3", "FTTH-1G", "--discount", "10", "--json", "2"].iter().map(|s| s.to_string()).collect();
        assert_eq!(positional(&args), vec!["quote", "add", "3", "FTTH-1G", "2"]);
        assert_eq!(flag(&args, "--discount").as_deref(), Some("10"));
        assert_eq!(pairs(&["x".into(), "title=Ciao".into(), "validity_days=15".into()], 1), json!({"title": "Ciao", "validity_days": 15}));
    }

    #[test]
    fn the_demo_cli_lists_and_quotes() {
        let demo: Vec<String> = vec!["--demo".into(), "--user".into(), "anna".into()];
        let mut args = demo.clone();
        args.insert(0, "items".into());
        assert!(run(&args).is_ok());
        let mut args = demo.clone();
        args.splice(0..0, ["quote".to_string(), "new".into(), "--channel".into(), "wholesale".into(), "--customer".into(), "2".into()]);
        assert!(run(&args).is_ok());
        let mut args = demo.clone();
        args.splice(0..0, ["quote".to_string(), "show".into(), "1".into(), "--html".into()]);
        assert!(run(&args).is_ok());
        let mut args = demo;
        args.insert(0, "nope".into());
        assert!(run(&args).is_err());
    }
}
