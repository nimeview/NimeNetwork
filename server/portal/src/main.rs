use std::{
    env, fmt, io,
    io::{Read, Write},
    net::{SocketAddr, TcpListener, TcpStream},
    process,
};

use serde_json::Value;

const DEFAULT_BIND: &str = "127.0.0.1:8080";
const DEFAULT_REGISTRY_API: &str = "127.0.0.1:4172";

fn main() {
    if let Err(error) = run(env::args().skip(1).collect()) {
        eprintln!("nime-portal error: {error}");
        process::exit(1);
    }
}

fn run(args: Vec<String>) -> Result<(), String> {
    let config = Config::from_args(args)?;

    println!(
        "nime-portal listening on {} using registry API {}",
        config.bind_addr, config.registry_api_addr
    );

    let listener = TcpListener::bind(config.bind_addr)
        .map_err(|error| format!("failed to bind {}: {error}", config.bind_addr))?;

    for stream in listener.incoming() {
        match stream {
            Ok(stream) => {
                if let Err(error) = handle_connection(stream, &config) {
                    eprintln!("portal request error: {error}");
                }
            }
            Err(error) => eprintln!("portal accept error: {error}"),
        }
    }

    Ok(())
}

#[derive(Debug)]
struct Config {
    bind_addr: SocketAddr,
    registry_api_addr: SocketAddr,
    dns_zone: String,
}

impl Config {
    fn from_args(args: Vec<String>) -> Result<Self, String> {
        let mut bind_addr: SocketAddr = DEFAULT_BIND
            .parse()
            .map_err(|_| String::from("invalid default bind"))?;
        let mut registry_api_addr: SocketAddr = DEFAULT_REGISTRY_API
            .parse()
            .map_err(|_| String::from("invalid default registry API bind"))?;
        let mut dns_zone = String::from("nime.internal");

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
                "--registry-api" => {
                    let value = args
                        .next()
                        .ok_or_else(|| String::from("missing value for --registry-api"))?;
                    registry_api_addr = value
                        .parse()
                        .map_err(|_| format!("invalid registry API address: {value}"))?;
                }
                "--dns-zone" => {
                    dns_zone = normalize_zone(
                        &args
                            .next()
                            .ok_or_else(|| String::from("missing value for --dns-zone"))?,
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
            registry_api_addr,
            dns_zone,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct Service {
    name: String,
    group_name: String,
    target_ip: String,
    port: u16,
    protocol: String,
    title: String,
    description: String,
    owner: String,
    tags: Vec<String>,
}

impl Service {
    fn internal_url(&self, dns_zone: &str) -> String {
        let fqdn = service_fqdn(self, dns_zone);
        default_service_url(&self.protocol, &fqdn, self.port)
    }
}

impl fmt::Display for Service {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}://{}:{}", self.protocol, self.target_ip, self.port)
    }
}

fn print_help() {
    println!("nime-portal options:");
    println!("  --bind 127.0.0.1:8080");
    println!("  --registry-api 127.0.0.1:4172");
    println!("  --dns-zone nime.internal");
}

fn handle_connection(mut stream: TcpStream, config: &Config) -> Result<(), String> {
    let mut buffer = [0_u8; 8192];
    let size = stream
        .read(&mut buffer)
        .map_err(|error| format!("failed to read request: {error}"))?;
    if size == 0 {
        return Ok(());
    }

    let request = String::from_utf8_lossy(&buffer[..size]);
    let line = request
        .lines()
        .next()
        .ok_or_else(|| String::from("empty request"))?;
    let mut parts = line.split_whitespace();
    let method = parts.next().unwrap_or("");
    let target = parts.next().unwrap_or("/");

    if method != "GET" {
        return write_response(
            &mut stream,
            "405 Method Not Allowed",
            "text/plain; charset=utf-8",
            "Only GET is supported",
        )
        .map_err(|error| error.to_string());
    }

    let (path, query) = split_path_and_query(target);
    match path {
        "/" => {
            let search_query = query_param(query, "q").unwrap_or_default();
            let services = load_services(&config.registry_api_addr, Some(search_query.as_str()))?;
            let body = render_page(&services, &config.dns_zone, Some(search_query.as_str()));
            write_response(&mut stream, "200 OK", "text/html; charset=utf-8", &body)
                .map_err(|error| error.to_string())
        }
        "/health" => write_response(&mut stream, "200 OK", "text/plain; charset=utf-8", "ok")
            .map_err(|error| error.to_string()),
        _ => write_response(
            &mut stream,
            "404 Not Found",
            "text/plain; charset=utf-8",
            "Not found",
        )
        .map_err(|error| error.to_string()),
    }
}

fn split_path_and_query(target: &str) -> (&str, &str) {
    target
        .split_once('?')
        .map_or((target, ""), |(path, query)| (path, query))
}

fn query_param(query: &str, key: &str) -> Option<String> {
    for pair in query.split('&') {
        let (k, v) = pair.split_once('=').unwrap_or((pair, ""));
        if k == key {
            return Some(percent_decode(v));
        }
    }
    None
}

fn percent_decode(input: &str) -> String {
    let mut out = String::new();
    let bytes = input.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        match bytes[i] {
            b'+' => {
                out.push(' ');
                i += 1;
            }
            b'%' if i + 2 < bytes.len() => {
                let hex = &input[i + 1..i + 3];
                if let Ok(value) = u8::from_str_radix(hex, 16) {
                    out.push(value as char);
                    i += 3;
                } else {
                    out.push('%');
                    i += 1;
                }
            }
            other => {
                out.push(other as char);
                i += 1;
            }
        }
    }
    out
}

fn load_services(
    registry_api_addr: &SocketAddr,
    query: Option<&str>,
) -> Result<Vec<Service>, String> {
    let mut target = String::from("/services");
    if let Some(query) = query.filter(|value| !value.trim().is_empty()) {
        target.push_str("?q=");
        target.push_str(&url_encode(query));
    }

    let body = http_get_json(*registry_api_addr, &target)?;
    let services = body
        .as_array()
        .ok_or_else(|| String::from("registry API returned unexpected payload"))?;

    let mut loaded = Vec::with_capacity(services.len());
    for service in services {
        loaded.push(service_from_json(service)?);
    }
    Ok(loaded)
}

fn service_from_json(service: &Value) -> Result<Service, String> {
    Ok(Service {
        name: service["name"]
            .as_str()
            .ok_or_else(|| String::from("service missing name"))?
            .to_owned(),
        group_name: service["group"]
            .as_str()
            .ok_or_else(|| String::from("service missing group"))?
            .to_owned(),
        target_ip: service["target_ip"]
            .as_str()
            .ok_or_else(|| String::from("service missing target_ip"))?
            .to_owned(),
        port: service["port"]
            .as_u64()
            .ok_or_else(|| String::from("service missing port"))? as u16,
        protocol: service["protocol"]
            .as_str()
            .ok_or_else(|| String::from("service missing protocol"))?
            .to_owned(),
        title: service["title"]
            .as_str()
            .ok_or_else(|| String::from("service missing title"))?
            .to_owned(),
        description: service["description"]
            .as_str()
            .ok_or_else(|| String::from("service missing description"))?
            .to_owned(),
        owner: service["owner"]
            .as_str()
            .ok_or_else(|| String::from("service missing owner"))?
            .to_owned(),
        tags: service["tags"]
            .as_array()
            .map(|tags| {
                tags.iter()
                    .filter_map(|tag| tag.as_str())
                    .map(str::to_owned)
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default(),
    })
}

fn service_fqdn(service: &Service, dns_zone: &str) -> String {
    format!("{}.{}.{}", service.name, service.group_name, dns_zone)
}

fn default_service_url(protocol: &str, fqdn: &str, port: u16) -> String {
    let default_port = (protocol == "http" && port == 80) || (protocol == "https" && port == 443);
    if default_port {
        format!("{protocol}://{fqdn}")
    } else {
        format!("{protocol}://{fqdn}:{port}")
    }
}

fn http_get_json(addr: SocketAddr, target: &str) -> Result<Value, String> {
    let mut stream = TcpStream::connect(addr)
        .map_err(|error| format!("failed to connect to registry API {addr}: {error}"))?;
    stream
        .set_read_timeout(Some(std::time::Duration::from_secs(2)))
        .map_err(|error| format!("failed to set read timeout: {error}"))?;
    stream
        .set_write_timeout(Some(std::time::Duration::from_secs(2)))
        .map_err(|error| format!("failed to set write timeout: {error}"))?;

    let request = format!("GET {target} HTTP/1.1\r\nHost: {addr}\r\nConnection: close\r\n\r\n");
    stream
        .write_all(request.as_bytes())
        .map_err(|error| format!("failed to write HTTP request: {error}"))?;

    let mut response = String::new();
    stream
        .read_to_string(&mut response)
        .map_err(|error| format!("failed to read HTTP response: {error}"))?;
    let (head, body) = response
        .split_once("\r\n\r\n")
        .ok_or_else(|| String::from("invalid HTTP response from registry API"))?;
    let status_line = head.lines().next().unwrap_or("");
    if !status_line.contains("200") {
        return Err(format!(
            "registry API returned unexpected status: {status_line}"
        ));
    }

    serde_json::from_str(body).map_err(|error| format!("invalid JSON from registry API: {error}"))
}

fn url_encode(input: &str) -> String {
    let mut encoded = String::new();
    for byte in input.bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                encoded.push(byte as char)
            }
            b' ' => encoded.push('+'),
            _ => {
                encoded.push('%');
                encoded.push_str(&format!("{byte:02X}"));
            }
        }
    }
    encoded
}

fn render_page(services: &[Service], dns_zone: &str, query: Option<&str>) -> String {
    let query = query.unwrap_or("");
    let mut cards = String::new();

    for service in services {
        let tags = if service.tags.is_empty() {
            String::from("<span class=\"tag\">untagged</span>")
        } else {
            service
                .tags
                .iter()
                .map(|tag| format!("<span class=\"tag\">{}</span>", escape_html(tag)))
                .collect::<Vec<_>>()
                .join("")
        };
        let description = if service.description.trim().is_empty() {
            String::from("No description yet.")
        } else {
            escape_html(&service.description)
        };
        let internal_url = service.internal_url(dns_zone);
        let fqdn = service_fqdn(service, dns_zone);
        cards.push_str(&format!(
            "<article class=\"card\">\
                <div class=\"card-top\">\
                    <div>\
                        <h2>{}</h2>\
                        <p class=\"fqdn\">{}</p>\
                    </div>\
                    <a class=\"open\" href=\"{}\">open</a>\
                </div>\
                <p class=\"desc\">{}</p>\
                <p class=\"meta\"><strong>Group:</strong> {} <strong>Owner:</strong> {} <strong>Target:</strong> {}://{}:{}</p>\
                <div class=\"tags\">{}</div>\
            </article>",
            escape_html(&service.title),
            escape_html(&fqdn),
            escape_html(&internal_url),
            description,
            escape_html(&service.group_name),
            escape_html(&service.owner),
            escape_html(&service.protocol),
            escape_html(&service.target_ip),
            service.port,
            tags
        ));
    }

    if cards.is_empty() {
        cards.push_str("<div class=\"empty\">No services found for this query.</div>");
    }

    format!(
        "<!doctype html>\
        <html lang=\"en\">\
        <head>\
          <meta charset=\"utf-8\">\
          <meta name=\"viewport\" content=\"width=device-width, initial-scale=1\">\
          <title>Nime Portal</title>\
          <style>\
            :root {{\
              --bg: #f3efe7;\
              --panel: rgba(255,255,255,0.82);\
              --text: #1d2a25;\
              --muted: #596861;\
              --accent: #0b7a75;\
              --accent-2: #d7a33d;\
              --line: rgba(29,42,37,0.12);\
            }}\
            * {{ box-sizing: border-box; }}\
            body {{\
              margin: 0;\
              font-family: Georgia, 'Iowan Old Style', serif;\
              color: var(--text);\
              background: radial-gradient(circle at top left, #fff9ef, #f1ecdf 42%, #dfe9e4 100%);\
            }}\
            .shell {{ max-width: 1100px; margin: 0 auto; padding: 40px 20px 80px; }}\
            .hero {{\
              padding: 28px;\
              border: 1px solid var(--line);\
              background: var(--panel);\
              backdrop-filter: blur(8px);\
              border-radius: 24px;\
              box-shadow: 0 16px 50px rgba(29,42,37,0.08);\
            }}\
            h1 {{ margin: 0 0 12px; font-size: clamp(2rem, 6vw, 4.4rem); line-height: 0.95; }}\
            .subtitle {{ margin: 0; color: var(--muted); font-size: 1.1rem; max-width: 62ch; }}\
            form {{ display: flex; gap: 12px; margin-top: 22px; flex-wrap: wrap; }}\
            input {{\
              flex: 1 1 320px;\
              padding: 14px 16px;\
              border-radius: 14px;\
              border: 1px solid var(--line);\
              background: rgba(255,255,255,0.9);\
              font-size: 1rem;\
            }}\
            button {{\
              padding: 14px 18px;\
              border-radius: 14px;\
              border: 0;\
              background: linear-gradient(135deg, var(--accent), #0f5f63);\
              color: white;\
              font-weight: 700;\
              cursor: pointer;\
            }}\
            .stats {{ display: flex; gap: 18px; flex-wrap: wrap; margin-top: 16px; color: var(--muted); }}\
            .grid {{ display: grid; grid-template-columns: repeat(auto-fit, minmax(260px, 1fr)); gap: 18px; margin-top: 28px; }}\
            .card {{\
              background: rgba(255,255,255,0.88);\
              border: 1px solid var(--line);\
              border-radius: 20px;\
              padding: 20px;\
              box-shadow: 0 14px 40px rgba(29,42,37,0.07);\
            }}\
            .card-top {{ display: flex; justify-content: space-between; gap: 16px; align-items: start; }}\
            .card h2 {{ margin: 0 0 6px; font-size: 1.3rem; }}\
            .fqdn {{ margin: 0; color: var(--muted); font-family: ui-monospace, SFMono-Regular, monospace; font-size: 0.92rem; }}\
            .desc {{ color: var(--text); min-height: 3em; }}\
            .meta {{ color: var(--muted); font-size: 0.95rem; line-height: 1.6; }}\
            .tags {{ display: flex; flex-wrap: wrap; gap: 8px; margin-top: 12px; }}\
            .tag {{\
              display: inline-block;\
              padding: 6px 10px;\
              border-radius: 999px;\
              background: #f2e2bb;\
              color: #5f4310;\
              font-size: 0.82rem;\
              font-weight: 700;\
            }}\
            .open {{\
              text-decoration: none;\
              color: white;\
              background: linear-gradient(135deg, var(--accent-2), #bb7a18);\
              padding: 10px 12px;\
              border-radius: 12px;\
              font-weight: 700;\
            }}\
            .empty {{\
              margin-top: 28px;\
              padding: 24px;\
              border-radius: 18px;\
              border: 1px dashed var(--line);\
              background: rgba(255,255,255,0.7);\
              color: var(--muted);\
            }}\
            @media (max-width: 700px) {{\
              .shell {{ padding: 20px 14px 60px; }}\
              .hero {{ padding: 22px; border-radius: 20px; }}\
              .card-top {{ flex-direction: column; }}\
            }}\
          </style>\
        </head>\
        <body>\
          <main class=\"shell\">\
            <section class=\"hero\">\
              <h1>Nime Network Portal</h1>\
              <p class=\"subtitle\">Private start page for services inside the mesh. Search by name, owner, tag, or description.</p>\
              <form method=\"GET\" action=\"/\">\
                <input type=\"text\" name=\"q\" placeholder=\"Search services, tags, owners...\" value=\"{}\">\
                <button type=\"submit\">Search</button>\
              </form>\
              <div class=\"stats\">\
                <span><strong>Zone:</strong> {}</span>\
                <span><strong>Services:</strong> {}</span>\
                <span><strong>Query:</strong> {}</span>\
              </div>\
            </section>\
            <section class=\"grid\">{}</section>\
          </main>\
        </body>\
        </html>",
        escape_html(query),
        escape_html(dns_zone),
        services.len(),
        if query.is_empty() {
            String::from("all")
        } else {
            escape_html(query)
        },
        cards
    )
}

fn escape_html(input: &str) -> String {
    input
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

fn write_response(
    stream: &mut TcpStream,
    status: &str,
    content_type: &str,
    body: &str,
) -> io::Result<()> {
    let response = format!(
        "HTTP/1.1 {status}\r\nContent-Type: {content_type}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
        body.len(),
        body
    );
    stream.write_all(response.as_bytes())
}

fn normalize_zone(zone: &str) -> String {
    zone.trim().trim_end_matches('.').to_ascii_lowercase()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decodes_query_values() {
        assert_eq!(
            query_param("q=hello+world", "q"),
            Some(String::from("hello world"))
        );
        assert_eq!(
            query_param("q=portal%20search", "q"),
            Some(String::from("portal search"))
        );
    }

    #[test]
    fn renders_internal_service_url() {
        let service = Service {
            name: String::from("portal"),
            group_name: String::from("core"),
            target_ip: String::from("10.42.0.1"),
            port: 80,
            protocol: String::from("http"),
            title: String::from("Portal"),
            description: String::new(),
            owner: String::from("arthur"),
            tags: vec![String::from("infra")],
        };

        assert_eq!(
            service.internal_url("nime.internal"),
            "http://portal.core.nime.internal"
        );
    }

    #[test]
    fn parses_service_from_registry_json() {
        let value = serde_json::json!({
            "name": "portal",
            "group": "core",
            "target_ip": "10.42.0.1",
            "port": 8080,
            "protocol": "http",
            "title": "Portal",
            "description": "Shared start page",
            "owner": "arthur",
            "tags": ["infra", "search"]
        });

        let service = service_from_json(&value).unwrap();
        assert_eq!(service.name, "portal");
        assert_eq!(service.group_name, "core");
        assert_eq!(service.target_ip, "10.42.0.1");
        assert_eq!(service.port, 8080);
        assert_eq!(service.tags, vec!["infra", "search"]);
    }
}
