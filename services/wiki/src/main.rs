use std::{
    env, fs, io,
    io::{Read, Write},
    net::{SocketAddr, TcpListener, TcpStream},
    path::{Path, PathBuf},
    process,
};

const DEFAULT_BIND: &str = "0.0.0.0:8181";

fn main() {
    if let Err(error) = run(env::args().skip(1).collect()) {
        eprintln!("nime-wiki error: {error}");
        process::exit(1);
    }
}

fn run(args: Vec<String>) -> Result<(), String> {
    let config = Config::from_args(args)?;
    fs::create_dir_all(&config.data_dir)
        .map_err(|error| format!("failed to create {}: {error}", config.data_dir.display()))?;
    ensure_home_page(&config.data_dir)?;

    println!(
        "nime-wiki listening on {} using {}",
        config.bind_addr,
        config.data_dir.display()
    );

    let listener = TcpListener::bind(config.bind_addr)
        .map_err(|error| format!("failed to bind {}: {error}", config.bind_addr))?;

    for stream in listener.incoming() {
        match stream {
            Ok(stream) => {
                if let Err(error) = handle_connection(stream, &config) {
                    eprintln!("wiki request error: {error}");
                }
            }
            Err(error) => eprintln!("wiki accept error: {error}"),
        }
    }

    Ok(())
}

#[derive(Debug)]
struct Config {
    bind_addr: SocketAddr,
    data_dir: PathBuf,
}

impl Config {
    fn from_args(args: Vec<String>) -> Result<Self, String> {
        let mut bind_addr: SocketAddr = DEFAULT_BIND
            .parse()
            .map_err(|_| String::from("invalid default bind"))?;
        let mut data_dir = PathBuf::from("services/data/wiki");
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
                "--data-dir" => {
                    data_dir = PathBuf::from(
                        args.next()
                            .ok_or_else(|| String::from("missing value for --data-dir"))?,
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
            data_dir,
        })
    }
}

fn print_help() {
    println!("nime-wiki options:");
    println!("  --bind 0.0.0.0:8181");
    println!("  --data-dir services/data/wiki");
}

fn ensure_home_page(data_dir: &Path) -> Result<(), String> {
    let path = home_page_path(data_dir);
    if !path.exists() {
        fs::write(
            &path,
            "# Nime Wiki\n\nЭто общая страница сети.\n\n- Добавь правила\n- Добавь инструкции\n- Добавь ссылки на сервисы\n",
        )
        .map_err(|error| format!("failed to initialize {}: {error}", path.display()))?;
    }
    Ok(())
}

fn home_page_path(data_dir: &Path) -> PathBuf {
    data_dir.join("home.md")
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
    let path = parts.next().unwrap_or("/");
    let body = &buffer[body_start..size];

    match (method, path) {
        ("GET", "/") => {
            let page = load_page(config)?;
            let body = render_page(&page);
            write_response(
                &mut stream,
                "200 OK",
                "text/html; charset=utf-8",
                body.as_bytes(),
            )
            .map_err(|error| error.to_string())
        }
        ("GET", "/raw") => {
            let page = load_page(config)?;
            write_response(
                &mut stream,
                "200 OK",
                "text/plain; charset=utf-8",
                page.as_bytes(),
            )
            .map_err(|error| error.to_string())
        }
        ("POST", "/page") => {
            let text = decode_form_body(body)?;
            let normalized = if let Some(value) = text.strip_prefix("body=") {
                url_decode(value)
            } else {
                text
            };
            let normalized = normalized.replace("\r\n", "\n");
            fs::write(
                home_page_path(&config.data_dir),
                normalized.trim().to_owned() + "\n",
            )
            .map_err(|error| format!("failed to write page: {error}"))?;
            write_response(&mut stream, "200 OK", "text/plain; charset=utf-8", b"ok\n")
                .map_err(|error| error.to_string())
        }
        ("GET", "/health") => {
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

fn decode_form_body(body: &[u8]) -> Result<String, String> {
    String::from_utf8(body.to_vec()).map_err(|_| String::from("page body must be valid utf-8"))
}

fn split_headers(request: &str) -> Result<(&str, usize), String> {
    request
        .find("\r\n\r\n")
        .map(|offset| (&request[..offset], offset + 4))
        .ok_or_else(|| String::from("invalid HTTP request"))
}

fn load_page(config: &Config) -> Result<String, String> {
    fs::read_to_string(home_page_path(&config.data_dir))
        .map_err(|error| format!("failed to read home page: {error}"))
}

fn render_page(markdown: &str) -> String {
    let escaped = escape_html(markdown);
    format!(
        "<!doctype html><html lang=\"ru\"><head><meta charset=\"utf-8\"><meta name=\"viewport\" content=\"width=device-width, initial-scale=1\"><title>Nime Wiki</title><style>body{{font-family:Georgia,serif;background:#f7f1e8;color:#1e2823;margin:0}}main{{max-width:920px;margin:0 auto;padding:32px 20px 64px}}textarea{{width:100%;min-height:280px;font:16px/1.5 monospace;padding:14px;border:1px solid #d2c7b7;background:#fffdf8}}button{{margin-top:12px;padding:10px 16px;border:0;background:#0b7a75;color:#fff;font-weight:700;cursor:pointer}}pre{{white-space:pre-wrap;background:#fffdf8;border:1px solid #d2c7b7;padding:16px}}</style></head><body><main><h1>Nime Wiki</h1><p>Общая страница сети. Правь её через форму ниже или через <code>POST /page</code>.</p><pre>{escaped}</pre><form method=\"post\" action=\"/page\"><textarea name=\"body\">{escaped}</textarea><br><button type=\"submit\">Сохранить</button></form></main></body></html>"
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn renders_page() {
        let html = render_page("# Wiki");
        assert!(html.contains("Nime Wiki"));
        assert!(html.contains("# Wiki"));
    }
}
