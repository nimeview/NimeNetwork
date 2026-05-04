use std::{
    collections::HashMap,
    env, io,
    io::{Read, Write},
    net::{Ipv4Addr, SocketAddr, TcpStream, UdpSocket},
    path::{Path, PathBuf},
    time::{Duration, Instant},
};

use rusqlite::Connection;
use serde_json::Value;

const TYPE_A: u16 = 1;
const CLASS_IN: u16 = 1;

fn main() {
    if let Err(error) = run() {
        eprintln!("nime-dns error: {error}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), String> {
    let config = Config::from_args(env::args().skip(1))?;
    let reload = load_record_set(&config)?;

    println!(
        "nime-dns listening on {} for zone {} using {} records{}{}; reload interval {}s",
        config.bind_addr,
        config.zone,
        reload.records.len(),
        if reload.innernet_peer_count > 0 {
            format!(", plus {} peers from innernet", reload.innernet_peer_count)
        } else {
            String::new()
        },
        if reload.registry_service_count > 0 {
            format!(
                ", plus {} services from registry",
                reload.registry_service_count
            )
        } else {
            String::new()
        },
        config.reload_interval.as_secs()
    );

    serve(config, reload.records).map_err(|error| error.to_string())
}

#[derive(Debug)]
struct Config {
    bind_addr: SocketAddr,
    zone: String,
    innernet_db_path: PathBuf,
    registry_api_addr: SocketAddr,
    reload_interval: Duration,
}

impl Config {
    fn from_args(args: impl Iterator<Item = String>) -> Result<Self, String> {
        let mut bind_addr = String::from("0.0.0.0:5354");
        let mut zone = String::from("nime.internal");
        let mut innernet_db_path = PathBuf::from("server/innernet/innernet.db");
        let mut registry_api_addr: SocketAddr = "127.0.0.1:4172"
            .parse()
            .map_err(|_| String::from("invalid default registry API address"))?;
        let mut reload_interval_secs = 5_u64;

        let mut args = args.peekable();
        while let Some(arg) = args.next() {
            match arg.as_str() {
                "--bind" => {
                    bind_addr = args
                        .next()
                        .ok_or_else(|| String::from("missing value for --bind"))?;
                }
                "--zone" => {
                    zone = args
                        .next()
                        .ok_or_else(|| String::from("missing value for --zone"))?;
                }
                "--innernet-db" => {
                    innernet_db_path = PathBuf::from(
                        args.next()
                            .ok_or_else(|| String::from("missing value for --innernet-db"))?,
                    );
                }
                "--registry-api" => {
                    let value = args
                        .next()
                        .ok_or_else(|| String::from("missing value for --registry-api"))?;
                    registry_api_addr = value
                        .parse()
                        .map_err(|_| format!("invalid registry API address: {value}"))?;
                }
                "--reload-secs" => {
                    let value = args
                        .next()
                        .ok_or_else(|| String::from("missing value for --reload-secs"))?;
                    reload_interval_secs = value
                        .parse()
                        .map_err(|_| format!("invalid reload interval: {value}"))?;
                }
                "--help" | "-h" => {
                    print_help();
                    std::process::exit(0);
                }
                other => {
                    return Err(format!("unknown argument: {other}"));
                }
            }
        }

        let bind_addr = bind_addr
            .parse()
            .map_err(|_| format!("invalid bind address: {bind_addr}"))?;
        let zone = normalize_name(&zone);
        if zone.is_empty() {
            return Err(String::from("zone must not be empty"));
        }

        Ok(Self {
            bind_addr,
            zone,
            innernet_db_path,
            registry_api_addr,
            reload_interval: Duration::from_secs(reload_interval_secs),
        })
    }
}

fn print_help() {
    println!(
        "Usage: nime-dns [--bind 0.0.0.0:5354] [--zone nime.internal] [--innernet-db /path/to/innernet.db] [--registry-api 127.0.0.1:4172] [--reload-secs 5]"
    );
}

struct RecordSet {
    records: HashMap<String, Ipv4Addr>,
    innernet_peer_count: usize,
    registry_service_count: usize,
}

fn load_record_set(config: &Config) -> Result<RecordSet, String> {
    let mut records = HashMap::new();
    let innernet_peer_count =
        merge_innernet_records(&mut records, &config.innernet_db_path, &config.zone)?;
    let registry_service_count =
        merge_registry_records_from_api(&mut records, config.registry_api_addr, &config.zone)?;

    Ok(RecordSet {
        records,
        innernet_peer_count,
        registry_service_count,
    })
}

fn merge_innernet_records(
    records: &mut HashMap<String, Ipv4Addr>,
    db_path: &Path,
    zone: &str,
) -> Result<usize, String> {
    let conn = Connection::open(db_path)
        .map_err(|error| format!("failed to open innernet db {}: {error}", db_path.display()))?;
    let mut stmt = conn
        .prepare(
            "SELECT name, ip
            FROM peers
            WHERE is_redeemed = 1 AND is_disabled = 0",
        )
        .map_err(|error| format!("failed to prepare innernet peer query: {error}"))?;
    let rows = stmt
        .query_map([], |row| {
            let name: String = row.get(0)?;
            let ip: String = row.get(1)?;
            Ok((name, ip))
        })
        .map_err(|error| format!("failed to query innernet peers: {error}"))?;

    let mut loaded = 0_usize;
    for row in rows {
        let (name, ip) =
            row.map_err(|error| format!("failed to read innernet peer row: {error}"))?;
        let fqdn = qualify_name(&name, zone)?;
        let ip: Ipv4Addr = match ip.parse() {
            Ok(ip) => ip,
            Err(_) => continue,
        };

        records.insert(fqdn, ip);
        loaded += 1;
    }

    Ok(loaded)
}

fn merge_registry_records_from_api(
    records: &mut HashMap<String, Ipv4Addr>,
    registry_api_addr: SocketAddr,
    zone: &str,
) -> Result<usize, String> {
    let body = http_get_json(registry_api_addr, "/services")?;
    let rows = body
        .as_array()
        .ok_or_else(|| String::from("registry API returned unexpected payload"))?;
    let mut loaded = 0_usize;
    let mut short_names = HashMap::<String, Vec<(String, Ipv4Addr)>>::new();
    for row in rows {
        let name = row["name"]
            .as_str()
            .ok_or_else(|| String::from("service missing name"))?;
        let group_name = row["group"]
            .as_str()
            .ok_or_else(|| String::from("service missing group"))?;
        let ip = row["target_ip"]
            .as_str()
            .ok_or_else(|| String::from("service missing target_ip"))?;
        let ip: Ipv4Addr = match ip.parse() {
            Ok(ip) => ip,
            Err(_) => continue,
        };
        let fqdn = qualify_name(&format!("{name}.{group_name}"), zone)?;

        records.insert(fqdn, ip);
        short_names
            .entry(normalize_name(name))
            .or_default()
            .push((group_name.to_owned(), ip));
        loaded += 1;
    }

    for (name, entries) in short_names {
        if entries.len() == 1 {
            let alias = qualify_name(&name, zone)?;
            records.insert(alias, entries[0].1);
        }
    }

    Ok(loaded)
}

fn http_get_json(addr: SocketAddr, target: &str) -> Result<Value, String> {
    let mut stream = TcpStream::connect(addr)
        .map_err(|error| format!("failed to connect to registry API {addr}: {error}"))?;
    stream
        .set_read_timeout(Some(Duration::from_secs(2)))
        .map_err(|error| format!("failed to set read timeout: {error}"))?;
    stream
        .set_write_timeout(Some(Duration::from_secs(2)))
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

fn qualify_name(name: &str, zone: &str) -> Result<String, String> {
    let normalized = normalize_name(name);
    if normalized.is_empty() {
        return Err(String::from("record name must not be empty"));
    }

    if normalized == "@" {
        return Ok(zone.to_owned());
    }

    if normalized == zone || normalized.ends_with(&format!(".{zone}")) {
        return Ok(normalized);
    }

    Ok(format!("{normalized}.{zone}"))
}

fn normalize_name(name: &str) -> String {
    name.trim().trim_end_matches('.').to_ascii_lowercase()
}

fn serve(config: Config, mut records: HashMap<String, Ipv4Addr>) -> io::Result<()> {
    let socket = UdpSocket::bind(config.bind_addr)?;
    socket.set_read_timeout(Some(Duration::from_secs(1)))?;
    let mut buffer = [0_u8; 512];
    let mut last_reload = Instant::now();

    loop {
        if last_reload.elapsed() >= config.reload_interval {
            match load_record_set(&config) {
                Ok(reload) => {
                    records = reload.records;
                    if reload.innernet_peer_count > 0 || reload.registry_service_count > 0 {
                        println!(
                            "reloaded DNS records: {} total entries, {} innernet peers, {} registry services",
                            records.len(),
                            reload.innernet_peer_count,
                            reload.registry_service_count
                        );
                    } else {
                        println!("reloaded DNS records: {} total entries", records.len());
                    }
                }
                Err(error) => eprintln!("failed to reload DNS records: {error}"),
            }
            last_reload = Instant::now();
        }

        let (size, peer) = match socket.recv_from(&mut buffer) {
            Ok(result) => result,
            Err(error)
                if matches!(
                    error.kind(),
                    io::ErrorKind::WouldBlock | io::ErrorKind::TimedOut
                ) =>
            {
                continue;
            }
            Err(error) => return Err(error),
        };
        let response = match handle_query(&buffer[..size], &config.zone, &records) {
            Ok(response) => response,
            Err(error) => {
                eprintln!("failed to handle DNS query from {peer}: {error}");
                continue;
            }
        };

        socket.send_to(&response, peer)?;
    }
}

fn handle_query(
    packet: &[u8],
    zone: &str,
    records: &HashMap<String, Ipv4Addr>,
) -> Result<Vec<u8>, String> {
    if packet.len() < 12 {
        return Err(String::from("packet too short"));
    }

    let id = read_u16(packet, 0)?;
    let flags = read_u16(packet, 2)?;
    let qdcount = read_u16(packet, 4)?;
    let opcode = (flags >> 11) & 0x0f;

    if opcode != 0 {
        return Ok(build_error_response(packet, id, 4));
    }

    if qdcount != 1 {
        return Ok(build_error_response(packet, id, 1));
    }

    let (question, question_end) = parse_question(packet)?;
    let normalized_name = normalize_name(&question.name);
    let mut rcode = 0_u16;
    let mut answer_ip = None;

    if normalized_name == zone || normalized_name.ends_with(&format!(".{zone}")) {
        if question.qtype == TYPE_A && question.qclass == CLASS_IN {
            answer_ip = records.get(&normalized_name).copied();
            if answer_ip.is_none() {
                rcode = 3;
            }
        }
    } else {
        rcode = 3;
    }

    Ok(build_response(packet, id, question_end, answer_ip, rcode))
}

#[derive(Debug, PartialEq, Eq)]
struct Question {
    name: String,
    qtype: u16,
    qclass: u16,
}

fn parse_question(packet: &[u8]) -> Result<(Question, usize), String> {
    let (name, offset) = decode_name(packet, 12)?;
    let qtype = read_u16(packet, offset)?;
    let qclass = read_u16(packet, offset + 2)?;

    Ok((
        Question {
            name,
            qtype,
            qclass,
        },
        offset + 4,
    ))
}

fn decode_name(packet: &[u8], start: usize) -> Result<(String, usize), String> {
    let mut labels = Vec::new();
    let mut offset = start;
    let mut consumed = None;
    let mut jumps = 0_u8;

    loop {
        let len = *packet
            .get(offset)
            .ok_or_else(|| String::from("unexpected end of packet while decoding name"))?;

        if len & 0b1100_0000 == 0b1100_0000 {
            let next = *packet
                .get(offset + 1)
                .ok_or_else(|| String::from("invalid compression pointer"))?;
            let pointer = (((len as u16 & 0x3f) << 8) | next as u16) as usize;

            if consumed.is_none() {
                consumed = Some(offset + 2);
            }

            offset = pointer;
            jumps = jumps.saturating_add(1);
            if jumps > 10 {
                return Err(String::from("too many compression jumps"));
            }
            continue;
        }

        offset += 1;
        if len == 0 {
            let end = consumed.unwrap_or(offset);
            return Ok((labels.join("."), end));
        }

        let label_end = offset + len as usize;
        let label = packet
            .get(offset..label_end)
            .ok_or_else(|| String::from("label exceeds packet length"))?;
        let label = std::str::from_utf8(label)
            .map_err(|_| String::from("label is not valid utf-8"))?
            .to_ascii_lowercase();
        labels.push(label);
        offset = label_end;
    }
}

fn build_error_response(packet: &[u8], id: u16, rcode: u16) -> Vec<u8> {
    let question_end = packet.len().min(512);
    build_response(packet, id, question_end, None, rcode)
}

fn build_response(
    packet: &[u8],
    id: u16,
    question_end: usize,
    answer_ip: Option<Ipv4Addr>,
    rcode: u16,
) -> Vec<u8> {
    let mut response = Vec::with_capacity(512);
    response.extend_from_slice(&id.to_be_bytes());

    let mut flags = 0x8000_u16 | 0x0400_u16;
    flags |= rcode & 0x000f;
    response.extend_from_slice(&flags.to_be_bytes());
    response.extend_from_slice(&1_u16.to_be_bytes());
    response.extend_from_slice(&(u16::from(answer_ip.is_some())).to_be_bytes());
    response.extend_from_slice(&0_u16.to_be_bytes());
    response.extend_from_slice(&0_u16.to_be_bytes());

    let question_slice = if packet.len() >= 12 && question_end >= 12 && question_end <= packet.len()
    {
        &packet[12..question_end]
    } else {
        &[]
    };
    response.extend_from_slice(question_slice);

    if let Some(ip) = answer_ip {
        response.extend_from_slice(&0xC00C_u16.to_be_bytes());
        response.extend_from_slice(&TYPE_A.to_be_bytes());
        response.extend_from_slice(&CLASS_IN.to_be_bytes());
        response.extend_from_slice(&60_u32.to_be_bytes());
        response.extend_from_slice(&4_u16.to_be_bytes());
        response.extend_from_slice(&ip.octets());
    }

    response
}

fn read_u16(packet: &[u8], offset: usize) -> Result<u16, String> {
    let bytes = packet
        .get(offset..offset + 2)
        .ok_or_else(|| String::from("unexpected end of packet"))?;
    Ok(u16::from_be_bytes([bytes[0], bytes[1]]))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn qualifies_relative_name() {
        assert_eq!(
            qualify_name("portal", "nime.internal").unwrap(),
            "portal.nime.internal"
        );
        assert_eq!(qualify_name("@", "nime.internal").unwrap(), "nime.internal");
    }

    #[test]
    fn parses_dns_question() {
        let packet = [
            0x12, 0x34, 0x01, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x06, b'p',
            b'o', b'r', b't', b'a', b'l', 0x04, b'n', b'i', b'm', b'e', 0x08, b'i', b'n', b't',
            b'e', b'r', b'n', b'a', b'l', 0x00, 0x00, 0x01, 0x00, 0x01,
        ];

        let (question, offset) = parse_question(&packet).unwrap();
        assert_eq!(
            question,
            Question {
                name: String::from("portal.nime.internal"),
                qtype: TYPE_A,
                qclass: CLASS_IN,
            }
        );
        assert_eq!(offset, packet.len());
    }

    #[test]
    fn answers_a_record() {
        let mut records = HashMap::new();
        records.insert(
            String::from("portal.nime.internal"),
            Ipv4Addr::new(10, 60, 0, 10),
        );

        let packet = [
            0x12, 0x34, 0x01, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x06, b'p',
            b'o', b'r', b't', b'a', b'l', 0x04, b'n', b'i', b'm', b'e', 0x08, b'i', b'n', b't',
            b'e', b'r', b'n', b'a', b'l', 0x00, 0x00, 0x01, 0x00, 0x01,
        ];

        let response = handle_query(&packet, "nime.internal", &records).unwrap();
        assert_eq!(&response[0..2], &[0x12, 0x34]);
        assert_eq!(u16::from_be_bytes([response[6], response[7]]), 1);
        assert_eq!(&response[response.len() - 4..], &[10, 60, 0, 10]);
    }

    #[test]
    fn prefers_innernet_style_names_as_fqdn() {
        assert_eq!(
            qualify_name("peer-1", "nime.internal").unwrap(),
            "peer-1.nime.internal"
        );
        assert_eq!(
            qualify_name("peer-1.nime.internal", "nime.internal").unwrap(),
            "peer-1.nime.internal"
        );
    }

    #[test]
    fn normalizes_zone_name() {
        assert_eq!(normalize_name("NIME.INTERNAL."), "nime.internal");
    }
}
