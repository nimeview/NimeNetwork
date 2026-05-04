use std::{
    env, fs, io,
    io::{Read, Write},
    net::{SocketAddr, TcpListener, TcpStream},
    path::{Path, PathBuf},
    process,
};

use serde_json::{Value, json};

const DEFAULT_BIND: &str = "0.0.0.0:8383";
const DEFAULT_SUPPLY: u64 = 1_000_000;

fn main() {
    if let Err(error) = run(env::args().skip(1).collect()) {
        eprintln!("nime-ledger error: {error}");
        process::exit(1);
    }
}

fn run(args: Vec<String>) -> Result<(), String> {
    let config = Config::from_args(args)?;
    if let Some(parent) = config.state_file.parent() {
        fs::create_dir_all(parent)
            .map_err(|error| format!("failed to create {}: {error}", parent.display()))?;
    }
    ensure_state(&config.state_file)?;

    println!(
        "nime-ledger listening on {} using {}",
        config.bind_addr,
        config.state_file.display()
    );

    let listener = TcpListener::bind(config.bind_addr)
        .map_err(|error| format!("failed to bind {}: {error}", config.bind_addr))?;

    for stream in listener.incoming() {
        match stream {
            Ok(stream) => {
                if let Err(error) = handle_connection(stream, &config) {
                    eprintln!("ledger request error: {error}");
                }
            }
            Err(error) => eprintln!("ledger accept error: {error}"),
        }
    }

    Ok(())
}

#[derive(Debug)]
struct Config {
    bind_addr: SocketAddr,
    state_file: PathBuf,
}

impl Config {
    fn from_args(args: Vec<String>) -> Result<Self, String> {
        let mut bind_addr: SocketAddr = DEFAULT_BIND
            .parse()
            .map_err(|_| String::from("invalid default bind"))?;
        let mut state_file = PathBuf::from("services/data/ledger/state.json");
        let mut args = args.into_iter();

        while let Some(arg) = args.next() {
            match arg.as_str() {
                "--bind" => {
                    let value = args
                        .next()
                        .ok_or_else(|| String::from("missing value for --bind"))?;
                    bind_addr = value
                        .parse()
                        .map_err(|_| format!("invalid bind address: {value}"))?;
                }
                "--state-file" => {
                    state_file = PathBuf::from(
                        args.next()
                            .ok_or_else(|| String::from("missing value for --state-file"))?,
                    );
                }
                "--help" | "-h" => {
                    print_help();
                    process::exit(0);
                }
                other => return Err(format!("unknown argument: {other}")),
            }
        }

        Ok(Self {
            bind_addr,
            state_file,
        })
    }
}

fn print_help() {
    println!("nime-ledger options:");
    println!("  --bind 0.0.0.0:8383");
    println!("  --state-file services/data/ledger/state.json");
}

#[derive(Debug, Clone)]
struct LedgerState {
    supply_cap: u64,
    issued_total: u64,
    currency_code: String,
    owner: String,
    note: String,
    accounts: Vec<Account>,
}

#[derive(Debug, Clone)]
struct Account {
    name: String,
    balance: u64,
}

fn ensure_state(path: &Path) -> Result<(), String> {
    if path.exists() {
        return Ok(());
    }

    let state = LedgerState {
        supply_cap: DEFAULT_SUPPLY,
        issued_total: 0,
        currency_code: String::from("NIM"),
        owner: String::from("nime"),
        note: String::from(
            "Внутренняя игровая валюта сети. Не связана с реальными деньгами и не имеет внешней стоимости.",
        ),
        accounts: vec![
            Account {
                name: String::from("treasury"),
                balance: 0,
            },
            Account {
                name: String::from("community"),
                balance: 0,
            },
        ],
    };
    save_state(path, &state)
}

fn handle_connection(mut stream: TcpStream, config: &Config) -> Result<(), String> {
    let mut buffer = vec![0_u8; 128 * 1024];
    let size = stream
        .read(&mut buffer)
        .map_err(|error| format!("failed to read request: {error}"))?;
    if size == 0 {
        return Ok(());
    }

    let request = String::from_utf8_lossy(&buffer[..size]);
    let (header_text, body_start) = split_headers(&request)?;
    let request_line = header_text
        .lines()
        .next()
        .ok_or_else(|| String::from("empty request"))?;
    let mut parts = request_line.split_whitespace();
    let method = parts.next().unwrap_or("");
    let target = parts.next().unwrap_or("/");
    let (path, query) = split_path_query(target);
    let body = &buffer[body_start..size];

    match (method, path) {
        ("GET", "/") => {
            let state = load_state(&config.state_file)?;
            let body = render_page(&state);
            write_response(
                &mut stream,
                "200 OK",
                "text/html; charset=utf-8",
                body.as_bytes(),
            )
            .map_err(|error| error.to_string())
        }
        ("GET", "/health") => {
            write_response(&mut stream, "200 OK", "text/plain; charset=utf-8", b"ok\n")
                .map_err(|error| error.to_string())
        }
        ("GET", "/api/ledger") => {
            let state = load_state(&config.state_file)?;
            let body = ledger_to_json(&state).to_string();
            write_response(&mut stream, "200 OK", "application/json", body.as_bytes())
                .map_err(|error| error.to_string())
        }
        ("POST", "/issue") => {
            let account = query_param(query, "account")
                .ok_or_else(|| String::from("issue requires ?account=<name>"))?;
            let amount = query_param(query, "amount")
                .ok_or_else(|| String::from("issue requires ?amount=<number>"))?
                .parse::<u64>()
                .map_err(|_| String::from("amount must be an integer"))?;
            let mut state = load_state(&config.state_file)?;
            issue_to_account(&mut state, &account, amount)?;
            save_state(&config.state_file, &state)?;
            write_response(
                &mut stream,
                "200 OK",
                "application/json",
                ledger_to_json(&state).to_string().as_bytes(),
            )
            .map_err(|error| error.to_string())
        }
        ("POST", "/note") => {
            let text = String::from_utf8(body.to_vec())
                .map_err(|_| String::from("note body must be valid utf-8"))?;
            let mut state = load_state(&config.state_file)?;
            state.note = text.trim().to_owned();
            save_state(&config.state_file, &state)?;
            write_response(&mut stream, "200 OK", "text/plain; charset=utf-8", b"ok\n")
                .map_err(|error| error.to_string())
        }
        _ => write_response(
            &mut stream,
            "404 Not Found",
            "text/plain; charset=utf-8",
            b"not found\n",
        )
        .map_err(|error| error.to_string()),
    }
}

fn split_headers(request: &str) -> Result<(&str, usize), String> {
    request
        .find("\r\n\r\n")
        .map(|offset| (&request[..offset], offset + 4))
        .ok_or_else(|| String::from("invalid HTTP request"))
}

fn split_path_query(target: &str) -> (&str, &str) {
    target.split_once('?').unwrap_or((target, ""))
}

fn query_param(query: &str, key: &str) -> Option<String> {
    for pair in query.split('&') {
        let (candidate_key, value) = pair.split_once('=').unwrap_or((pair, ""));
        if candidate_key == key {
            return Some(url_decode(value));
        }
    }
    None
}

fn url_decode(value: &str) -> String {
    let mut decoded = String::new();
    let bytes = value.as_bytes();
    let mut index = 0;
    while index < bytes.len() {
        match bytes[index] {
            b'+' => {
                decoded.push(' ');
                index += 1;
            }
            b'%' if index + 2 < bytes.len() => {
                let hex = &value[index + 1..index + 3];
                if let Ok(byte) = u8::from_str_radix(hex, 16) {
                    decoded.push(byte as char);
                    index += 3;
                } else {
                    decoded.push('%');
                    index += 1;
                }
            }
            byte => {
                decoded.push(byte as char);
                index += 1;
            }
        }
    }
    decoded
}

fn load_state(path: &Path) -> Result<LedgerState, String> {
    let value: Value = serde_json::from_str(
        &fs::read_to_string(path)
            .map_err(|error| format!("failed to read {}: {error}", path.display()))?,
    )
    .map_err(|error| format!("failed to parse ledger state: {error}"))?;
    ledger_from_json(&value)
}

fn save_state(path: &Path, state: &LedgerState) -> Result<(), String> {
    let value = ledger_to_json(state);
    fs::write(
        path,
        serde_json::to_string_pretty(&value)
            .map_err(|error| format!("failed to serialize ledger state: {error}"))?,
    )
    .map_err(|error| format!("failed to write {}: {error}", path.display()))
}

fn issue_to_account(
    state: &mut LedgerState,
    account_name: &str,
    amount: u64,
) -> Result<(), String> {
    if amount == 0 {
        return Err(String::from("amount must be greater than zero"));
    }
    let next_total = state
        .issued_total
        .checked_add(amount)
        .ok_or_else(|| String::from("issued total overflow"))?;
    if next_total > state.supply_cap {
        return Err(String::from("issue would exceed supply cap"));
    }
    state.issued_total = next_total;

    if let Some(account) = state
        .accounts
        .iter_mut()
        .find(|account| account.name == account_name)
    {
        account.balance = account
            .balance
            .checked_add(amount)
            .ok_or_else(|| String::from("account balance overflow"))?;
    } else {
        state.accounts.push(Account {
            name: account_name.to_owned(),
            balance: amount,
        });
        state
            .accounts
            .sort_by(|left, right| left.name.cmp(&right.name));
    }

    Ok(())
}

fn ledger_to_json(state: &LedgerState) -> Value {
    json!({
        "currency_code": state.currency_code,
        "owner": state.owner,
        "note": state.note,
        "supply_cap": state.supply_cap,
        "issued_total": state.issued_total,
        "remaining_supply": state.supply_cap.saturating_sub(state.issued_total),
        "accounts": state.accounts.iter().map(|account| {
            json!({
                "name": account.name,
                "balance": account.balance,
            })
        }).collect::<Vec<_>>(),
    })
}

fn ledger_from_json(value: &Value) -> Result<LedgerState, String> {
    let accounts = value["accounts"]
        .as_array()
        .ok_or_else(|| String::from("ledger state missing accounts"))?
        .iter()
        .map(|account| {
            Ok(Account {
                name: account["name"]
                    .as_str()
                    .ok_or_else(|| String::from("account missing name"))?
                    .to_owned(),
                balance: account["balance"]
                    .as_u64()
                    .ok_or_else(|| String::from("account missing balance"))?,
            })
        })
        .collect::<Result<Vec<_>, String>>()?;

    Ok(LedgerState {
        supply_cap: value["supply_cap"]
            .as_u64()
            .ok_or_else(|| String::from("ledger state missing supply_cap"))?,
        issued_total: value["issued_total"]
            .as_u64()
            .ok_or_else(|| String::from("ledger state missing issued_total"))?,
        currency_code: value["currency_code"]
            .as_str()
            .ok_or_else(|| String::from("ledger state missing currency_code"))?
            .to_owned(),
        owner: value["owner"]
            .as_str()
            .ok_or_else(|| String::from("ledger state missing owner"))?
            .to_owned(),
        note: value["note"]
            .as_str()
            .ok_or_else(|| String::from("ledger state missing note"))?
            .to_owned(),
        accounts,
    })
}

fn render_page(state: &LedgerState) -> String {
    let accounts = if state.accounts.is_empty() {
        String::from("<li>Пока нет счетов</li>")
    } else {
        state
            .accounts
            .iter()
            .map(|account| {
                format!(
                    "<li><strong>{}</strong> — {} {}</li>",
                    escape_html(&account.name),
                    account.balance,
                    escape_html(&state.currency_code)
                )
            })
            .collect::<Vec<_>>()
            .join("")
    };

    format!(
        "<!doctype html><html lang=\"ru\"><head><meta charset=\"utf-8\"><meta name=\"viewport\" content=\"width=device-width, initial-scale=1\"><title>Nime Ledger</title><style>body{{font-family:Georgia,serif;background:#f8efe3;color:#231d17;margin:0}}main{{max-width:920px;margin:0 auto;padding:32px 20px 64px}}code,pre{{background:#fff8f1;border:1px solid #e2cfb6;padding:2px 6px}}li{{margin:10px 0}}</style></head><body><main><h1>Nime Ledger</h1><p>{}</p><p><strong>Владелец:</strong> {} <strong>Валюта:</strong> {} <strong>Лимит:</strong> {} <strong>Выпущено:</strong> {} <strong>Осталось:</strong> {}</p><h2>Счета</h2><ul>{}</ul><h2>Выпуск</h2><pre>curl -X POST \"http://ledger.apps.nime.internal:8383/issue?account=community&amp;amount=100\"</pre></main></body></html>",
        escape_html(&state.note),
        escape_html(&state.owner),
        escape_html(&state.currency_code),
        state.supply_cap,
        state.issued_total,
        state.supply_cap.saturating_sub(state.issued_total),
        accounts
    )
}

fn write_response(
    stream: &mut TcpStream,
    status: &str,
    content_type: &str,
    body: &[u8],
) -> io::Result<()> {
    let headers = format!(
        "HTTP/1.1 {status}\r\nContent-Type: {content_type}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
        body.len()
    );
    stream.write_all(headers.as_bytes())?;
    stream.write_all(body)
}

fn escape_html(input: &str) -> String {
    input
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn respects_supply_cap() {
        let mut state = LedgerState {
            supply_cap: 10,
            issued_total: 8,
            currency_code: String::from("NIM"),
            owner: String::from("nime"),
            note: String::new(),
            accounts: vec![],
        };
        assert!(issue_to_account(&mut state, "community", 3).is_err());
        issue_to_account(&mut state, "community", 2).unwrap();
        assert_eq!(state.issued_total, 10);
    }
}
