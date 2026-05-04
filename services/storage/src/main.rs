use std::{
    env, fs, io,
    io::{Read, Write},
    net::{SocketAddr, TcpListener, TcpStream},
    path::{Path, PathBuf},
    process,
};

const DEFAULT_BIND: &str = "0.0.0.0:8282";

fn main() {
    if let Err(error) = run(env::args().skip(1).collect()) {
        eprintln!("nime-storage error: {error}");
        process::exit(1);
    }
}

fn run(args: Vec<String>) -> Result<(), String> {
    let config = Config::from_args(args)?;
    fs::create_dir_all(&config.data_dir)
        .map_err(|error| format!("failed to create {}: {error}", config.data_dir.display()))?;

    println!(
        "nime-storage listening on {} using {}",
        config.bind_addr,
        config.data_dir.display()
    );

    let listener = TcpListener::bind(config.bind_addr)
        .map_err(|error| format!("failed to bind {}: {error}", config.bind_addr))?;

    for stream in listener.incoming() {
        match stream {
            Ok(stream) => {
                if let Err(error) = handle_connection(stream, &config) {
                    eprintln!("storage request error: {error}");
                }
            }
            Err(error) => eprintln!("storage accept error: {error}"),
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
        let mut data_dir = PathBuf::from("services/data/storage");
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
    println!("nime-storage options:");
    println!("  --bind 0.0.0.0:8282");
    println!("  --data-dir services/data/storage");
}

fn handle_connection(mut stream: TcpStream, config: &Config) -> Result<(), String> {
    let mut buffer = vec![0_u8; 2 * 1024 * 1024];
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
            let body = render_index(&config.data_dir)?;
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
        ("GET", path) if path.starts_with("/files/") => {
            let name = normalize_file_name(path.trim_start_matches("/files/"))?;
            let file_path = config.data_dir.join(&name);
            let contents = fs::read(&file_path)
                .map_err(|error| format!("failed to read {}: {error}", file_path.display()))?;
            write_response(&mut stream, "200 OK", "application/octet-stream", &contents)
                .map_err(|error| error.to_string())
        }
        ("POST", "/upload") => {
            let name = query_param(query, "name")
                .ok_or_else(|| String::from("upload requires ?name=<file>"))?;
            let name = normalize_file_name(&name)?;
            fs::write(config.data_dir.join(&name), body)
                .map_err(|error| format!("failed to write {name}: {error}"))?;
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

fn render_index(data_dir: &Path) -> Result<String, String> {
    let mut files = Vec::new();
    for entry in fs::read_dir(data_dir).map_err(|error| format!("failed to read dir: {error}"))? {
        let entry = entry.map_err(|error| format!("failed to read entry: {error}"))?;
        if entry
            .file_type()
            .map_err(|error| format!("failed to read file type: {error}"))?
            .is_file()
        {
            let name = entry.file_name().to_string_lossy().to_string();
            let size = entry
                .metadata()
                .map_err(|error| format!("failed to read metadata: {error}"))?
                .len();
            files.push((name, size));
        }
    }
    files.sort_by(|left, right| left.0.cmp(&right.0));

    let list = if files.is_empty() {
        String::from("<li>Пока нет файлов</li>")
    } else {
        files
            .iter()
            .map(|(name, size)| {
                format!(
                    "<li><a href=\"/files/{0}\">{0}</a> <span>{1} bytes</span></li>",
                    escape_html(name),
                    size
                )
            })
            .collect::<Vec<_>>()
            .join("")
    };

    Ok(format!(
        "<!doctype html><html lang=\"ru\"><head><meta charset=\"utf-8\"><meta name=\"viewport\" content=\"width=device-width, initial-scale=1\"><title>Nime Storage</title><style>body{{font-family:Georgia,serif;background:#edf3ef;color:#1e2823;margin:0}}main{{max-width:920px;margin:0 auto;padding:32px 20px 64px}}code,pre{{background:#f8fbf8;border:1px solid #c7d8cc;padding:2px 6px}}li{{margin:10px 0}}</style></head><body><main><h1>Nime Storage</h1><p>Общее файловое хранилище. Загрузка файлов делается raw POST-запросом:</p><pre>curl --data-binary @file.zip http://drive.apps.nime.internal:8282/upload?name=file.zip</pre><h2>Файлы</h2><ul>{list}</ul></main></body></html>"
    ))
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

fn normalize_file_name(name: &str) -> Result<String, String> {
    let trimmed = name.trim();
    if trimmed.is_empty() {
        return Err(String::from("file name must not be empty"));
    }
    if trimmed.contains('/') || trimmed.contains('\\') || trimmed.contains("..") {
        return Err(String::from("invalid file name"));
    }
    Ok(trimmed.to_owned())
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
    fn rejects_path_traversal() {
        assert!(normalize_file_name("../secret").is_err());
        assert_eq!(normalize_file_name("hello.txt").unwrap(), "hello.txt");
    }
}
