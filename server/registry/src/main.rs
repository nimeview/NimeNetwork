use std::{
    env, fmt, fs,
    io::{Read, Write},
    net::{Ipv4Addr, SocketAddr, TcpListener, TcpStream},
    path::{Path, PathBuf},
    process,
    time::{SystemTime, UNIX_EPOCH},
};

use rusqlite::{Connection, OptionalExtension, params};
use serde_json::{Value, json};

const DEFAULT_DB_PATH: &str = "server/registry/nime-registry.db";

fn main() {
    if let Err(error) = run(env::args().skip(1).collect()) {
        eprintln!("nime-registry error: {error}");
        process::exit(1);
    }
}

fn run(args: Vec<String>) -> Result<(), String> {
    match parse_cli(args)? {
        Command::InitDb { db_path } => {
            let conn = open_db(&db_path)?;
            init_schema(&conn)?;
            println!("initialized registry db at {}", db_path.display());
        }
        Command::Add {
            db_path,
            name,
            group_name,
            target_ip,
            port,
            protocol,
            title,
            description,
            owner,
            tags,
        } => {
            let conn = open_db(&db_path)?;
            init_schema(&conn)?;
            let service = NewService {
                name,
                group_name,
                target_ip,
                port,
                protocol,
                title,
                description,
                owner,
                tags,
            };
            insert_service(&conn, &service)?;
            println!(
                "added service {} -> {}:{}",
                service.name, service.target_ip, service.port
            );
        }
        Command::Update {
            db_path,
            name,
            group_name,
            target_ip,
            port,
            protocol,
            title,
            description,
            owner,
            tags,
        } => {
            let conn = open_db(&db_path)?;
            init_schema(&conn)?;
            let update = UpdateService {
                group_name,
                target_ip,
                port,
                protocol,
                title,
                description,
                owner,
                tags,
            };
            let service = update_service(&conn, &name, &update)?;
            println!(
                "updated service {} -> {}:{}",
                service.name, service.target_ip, service.port
            );
        }
        Command::List { db_path } => {
            let conn = open_db(&db_path)?;
            init_schema(&conn)?;
            for service in list_services(&conn)? {
                println!("{service}");
            }
        }
        Command::Get { db_path, name } => {
            let conn = open_db(&db_path)?;
            init_schema(&conn)?;
            let service =
                get_service(&conn, &name)?.ok_or_else(|| format!("service `{name}` not found"))?;
            print_service(&service);
        }
        Command::Remove { db_path, name } => {
            let conn = open_db(&db_path)?;
            init_schema(&conn)?;
            let deleted = delete_service(&conn, &name)?;
            if deleted == 0 {
                return Err(format!("service `{name}` not found"));
            }
            println!("removed service {name}");
        }
        Command::ExportDns { db_path } => {
            let conn = open_db(&db_path)?;
            init_schema(&conn)?;
            for service in list_services(&conn)? {
                println!(
                    "{}.{} {}",
                    service.name, service.group_name, service.target_ip
                );
            }
        }
        Command::Bootstrap {
            db_path,
            portal_ip,
            portal_port,
            owner,
            title,
            description,
        } => {
            let conn = open_db(&db_path)?;
            init_schema(&conn)?;
            let portal = NewService {
                name: String::from("portal"),
                group_name: String::from("core"),
                target_ip: portal_ip,
                port: portal_port,
                protocol: String::from("http"),
                title,
                description,
                owner,
                tags: vec![String::from("infra"), String::from("search")],
            };
            upsert_service(&conn, &portal)?;
            println!(
                "bootstrapped registry at {} with portal -> {}:{}",
                db_path.display(),
                portal.target_ip,
                portal.port
            );
        }
        Command::Serve { db_path, bind } => {
            run_http_api(&db_path, bind)?;
        }
        Command::Help => print_help(),
    }

    Ok(())
}

#[derive(Debug)]
enum Command {
    InitDb {
        db_path: PathBuf,
    },
    Add {
        db_path: PathBuf,
        name: String,
        group_name: String,
        target_ip: Ipv4Addr,
        port: u16,
        protocol: String,
        title: String,
        description: String,
        owner: String,
        tags: Vec<String>,
    },
    Update {
        db_path: PathBuf,
        name: String,
        group_name: Option<String>,
        target_ip: Option<Ipv4Addr>,
        port: Option<u16>,
        protocol: Option<String>,
        title: Option<String>,
        description: Option<String>,
        owner: Option<String>,
        tags: Option<Vec<String>>,
    },
    List {
        db_path: PathBuf,
    },
    Get {
        db_path: PathBuf,
        name: String,
    },
    Remove {
        db_path: PathBuf,
        name: String,
    },
    ExportDns {
        db_path: PathBuf,
    },
    Bootstrap {
        db_path: PathBuf,
        portal_ip: Ipv4Addr,
        portal_port: u16,
        owner: String,
        title: String,
        description: String,
    },
    Serve {
        db_path: PathBuf,
        bind: SocketAddr,
    },
    Help,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct Service {
    name: String,
    group_name: String,
    target_ip: Ipv4Addr,
    port: u16,
    protocol: String,
    title: String,
    description: String,
    owner: String,
    tags: Vec<String>,
    created_at: u64,
}

#[derive(Debug)]
struct NewService {
    name: String,
    group_name: String,
    target_ip: Ipv4Addr,
    port: u16,
    protocol: String,
    title: String,
    description: String,
    owner: String,
    tags: Vec<String>,
}

#[derive(Debug)]
struct UpdateService {
    group_name: Option<String>,
    target_ip: Option<Ipv4Addr>,
    port: Option<u16>,
    protocol: Option<String>,
    title: Option<String>,
    description: Option<String>,
    owner: Option<String>,
    tags: Option<Vec<String>>,
}

impl fmt::Display for Service {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{}.{} {}://{}:{} owner={} tags={}",
            self.name,
            self.group_name,
            self.protocol,
            self.target_ip,
            self.port,
            self.owner,
            if self.tags.is_empty() {
                String::from("-")
            } else {
                self.tags.join(",")
            }
        )
    }
}

fn parse_cli(args: Vec<String>) -> Result<Command, String> {
    let mut args = args.into_iter();
    let Some(command) = args.next() else {
        return Ok(Command::Help);
    };

    match command.as_str() {
        "init-db" => parse_db_only(args.collect(), DbOnlyKind::InitDb),
        "list" => parse_db_only(args.collect(), DbOnlyKind::List),
        "export-dns" => parse_db_only(args.collect(), DbOnlyKind::ExportDns),
        "bootstrap" => parse_bootstrap(args.collect()),
        "serve" => parse_serve(args.collect()),
        "get" => parse_name_command(args.collect(), NameOnlyKind::Get),
        "remove" => parse_name_command(args.collect(), NameOnlyKind::Remove),
        "add" => parse_add(args.collect()),
        "update" => parse_update(args.collect()),
        "--help" | "-h" | "help" => Ok(Command::Help),
        other => Err(format!("unknown command: {other}")),
    }
}

enum DbOnlyKind {
    InitDb,
    List,
    ExportDns,
}

enum NameOnlyKind {
    Get,
    Remove,
}

fn parse_db_only(args: Vec<String>, kind: DbOnlyKind) -> Result<Command, String> {
    let db_path = parse_db_path(args)?;
    Ok(match kind {
        DbOnlyKind::InitDb => Command::InitDb { db_path },
        DbOnlyKind::List => Command::List { db_path },
        DbOnlyKind::ExportDns => Command::ExportDns { db_path },
    })
}

fn parse_name_command(args: Vec<String>, kind: NameOnlyKind) -> Result<Command, String> {
    let mut db_path = default_db_path();
    let mut name = None;
    let mut args = args.into_iter();

    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--db" => {
                db_path = PathBuf::from(
                    args.next()
                        .ok_or_else(|| String::from("missing value for --db"))?,
                );
            }
            "--name" => {
                name = Some(
                    args.next()
                        .ok_or_else(|| String::from("missing value for --name"))?,
                );
            }
            "--help" | "-h" => return Ok(Command::Help),
            other => return Err(format!("unknown argument: {other}")),
        }
    }

    let name = normalize_name(&name.ok_or_else(|| String::from("missing required --name"))?)?;

    Ok(match kind {
        NameOnlyKind::Get => Command::Get { db_path, name },
        NameOnlyKind::Remove => Command::Remove { db_path, name },
    })
}

fn parse_add(args: Vec<String>) -> Result<Command, String> {
    let mut db_path = default_db_path();
    let mut name = None;
    let mut group_name = String::from("apps");
    let mut target_ip = None;
    let mut port = 80_u16;
    let mut protocol = String::from("http");
    let mut title = None;
    let mut description = String::new();
    let mut owner = String::from("unknown");
    let mut tags = Vec::new();
    let mut args = args.into_iter();

    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--db" => {
                db_path = PathBuf::from(
                    args.next()
                        .ok_or_else(|| String::from("missing value for --db"))?,
                );
            }
            "--name" => {
                name = Some(
                    args.next()
                        .ok_or_else(|| String::from("missing value for --name"))?,
                );
            }
            "--group" => {
                group_name = args
                    .next()
                    .ok_or_else(|| String::from("missing value for --group"))?;
            }
            "--target-ip" => {
                let value = args
                    .next()
                    .ok_or_else(|| String::from("missing value for --target-ip"))?;
                target_ip = Some(
                    value
                        .parse()
                        .map_err(|_| format!("invalid IPv4 address: {value}"))?,
                );
            }
            "--port" => {
                let value = args
                    .next()
                    .ok_or_else(|| String::from("missing value for --port"))?;
                port = value
                    .parse()
                    .map_err(|_| format!("invalid port: {value}"))?;
            }
            "--proto" => {
                protocol = args
                    .next()
                    .ok_or_else(|| String::from("missing value for --proto"))?;
            }
            "--title" => {
                title = Some(
                    args.next()
                        .ok_or_else(|| String::from("missing value for --title"))?,
                );
            }
            "--description" => {
                description = args
                    .next()
                    .ok_or_else(|| String::from("missing value for --description"))?;
            }
            "--owner" => {
                owner = args
                    .next()
                    .ok_or_else(|| String::from("missing value for --owner"))?;
            }
            "--tags" => {
                let value = args
                    .next()
                    .ok_or_else(|| String::from("missing value for --tags"))?;
                tags = parse_tags(&value);
            }
            "--help" | "-h" => return Ok(Command::Help),
            other => return Err(format!("unknown argument: {other}")),
        }
    }

    let name = normalize_name(&name.ok_or_else(|| String::from("missing required --name"))?)?;
    let group_name = normalize_name(&group_name)?;
    let title = title.unwrap_or_else(|| name.clone());
    validate_non_empty(&protocol, "proto")?;
    validate_non_empty(&title, "title")?;
    validate_non_empty(&owner, "owner")?;

    Ok(Command::Add {
        db_path,
        name,
        group_name,
        target_ip: target_ip.ok_or_else(|| String::from("missing required --target-ip"))?,
        port,
        protocol,
        title,
        description,
        owner,
        tags,
    })
}

fn parse_bootstrap(args: Vec<String>) -> Result<Command, String> {
    let mut db_path = default_db_path();
    let mut portal_ip = None;
    let mut portal_port = 8080_u16;
    let mut owner = String::from("admin");
    let mut title = String::from("Nime Portal");
    let mut description = String::from("Internal start page for the Nime network");
    let mut args = args.into_iter();

    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--db" => {
                db_path = PathBuf::from(
                    args.next()
                        .ok_or_else(|| String::from("missing value for --db"))?,
                );
            }
            "--portal-ip" => {
                let value = args
                    .next()
                    .ok_or_else(|| String::from("missing value for --portal-ip"))?;
                portal_ip = Some(
                    value
                        .parse()
                        .map_err(|_| format!("invalid IPv4 address: {value}"))?,
                );
            }
            "--portal-port" => {
                let value = args
                    .next()
                    .ok_or_else(|| String::from("missing value for --portal-port"))?;
                portal_port = value
                    .parse()
                    .map_err(|_| format!("invalid port: {value}"))?;
            }
            "--owner" => {
                owner = args
                    .next()
                    .ok_or_else(|| String::from("missing value for --owner"))?;
            }
            "--title" => {
                title = args
                    .next()
                    .ok_or_else(|| String::from("missing value for --title"))?;
            }
            "--description" => {
                description = args
                    .next()
                    .ok_or_else(|| String::from("missing value for --description"))?;
            }
            "--help" | "-h" => return Ok(Command::Help),
            other => return Err(format!("unknown argument: {other}")),
        }
    }

    Ok(Command::Bootstrap {
        db_path,
        portal_ip: portal_ip.ok_or_else(|| String::from("missing required --portal-ip"))?,
        portal_port,
        owner,
        title,
        description,
    })
}

fn parse_update(args: Vec<String>) -> Result<Command, String> {
    let mut db_path = default_db_path();
    let mut name = None;
    let mut group_name = None;
    let mut target_ip = None;
    let mut port = None;
    let mut protocol = None;
    let mut title = None;
    let mut description = None;
    let mut owner = None;
    let mut tags = None;
    let mut args = args.into_iter();

    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--db" => {
                db_path = PathBuf::from(
                    args.next()
                        .ok_or_else(|| String::from("missing value for --db"))?,
                );
            }
            "--name" => {
                name = Some(
                    args.next()
                        .ok_or_else(|| String::from("missing value for --name"))?,
                );
            }
            "--group" => {
                group_name = Some(normalize_name(
                    &args
                        .next()
                        .ok_or_else(|| String::from("missing value for --group"))?,
                )?);
            }
            "--target-ip" => {
                let value = args
                    .next()
                    .ok_or_else(|| String::from("missing value for --target-ip"))?;
                target_ip = Some(
                    value
                        .parse()
                        .map_err(|_| format!("invalid IPv4 address: {value}"))?,
                );
            }
            "--port" => {
                let value = args
                    .next()
                    .ok_or_else(|| String::from("missing value for --port"))?;
                port = Some(
                    value
                        .parse()
                        .map_err(|_| format!("invalid port: {value}"))?,
                );
            }
            "--proto" => {
                protocol = Some(
                    args.next()
                        .ok_or_else(|| String::from("missing value for --proto"))?,
                );
            }
            "--title" => {
                title = Some(
                    args.next()
                        .ok_or_else(|| String::from("missing value for --title"))?,
                );
            }
            "--description" => {
                description = Some(
                    args.next()
                        .ok_or_else(|| String::from("missing value for --description"))?,
                );
            }
            "--owner" => {
                owner = Some(
                    args.next()
                        .ok_or_else(|| String::from("missing value for --owner"))?,
                );
            }
            "--tags" => {
                let value = args
                    .next()
                    .ok_or_else(|| String::from("missing value for --tags"))?;
                tags = Some(parse_tags(&value));
            }
            "--help" | "-h" => return Ok(Command::Help),
            other => return Err(format!("unknown argument: {other}")),
        }
    }

    if let Some(protocol) = &protocol {
        validate_non_empty(protocol, "proto")?;
    }
    if let Some(title) = &title {
        validate_non_empty(title, "title")?;
    }
    if let Some(owner) = &owner {
        validate_non_empty(owner, "owner")?;
    }

    Ok(Command::Update {
        db_path,
        name: normalize_name(&name.ok_or_else(|| String::from("missing required --name"))?)?,
        group_name,
        target_ip,
        port,
        protocol,
        title,
        description,
        owner,
        tags,
    })
}

fn parse_serve(args: Vec<String>) -> Result<Command, String> {
    let mut db_path = default_db_path();
    let mut bind: SocketAddr = "127.0.0.1:4172"
        .parse()
        .map_err(|_| String::from("invalid default bind address"))?;
    let mut args = args.into_iter();

    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--db" => {
                db_path = PathBuf::from(
                    args.next()
                        .ok_or_else(|| String::from("missing value for --db"))?,
                );
            }
            "--bind" => {
                let value = args
                    .next()
                    .ok_or_else(|| String::from("missing value for --bind"))?;
                bind = value
                    .parse()
                    .map_err(|_| format!("invalid bind address: {value}"))?;
            }
            "--help" | "-h" => return Ok(Command::Help),
            other => return Err(format!("unknown argument: {other}")),
        }
    }

    Ok(Command::Serve { db_path, bind })
}

fn parse_db_path(args: Vec<String>) -> Result<PathBuf, String> {
    let mut db_path = default_db_path();
    let mut args = args.into_iter();

    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--db" => {
                db_path = PathBuf::from(
                    args.next()
                        .ok_or_else(|| String::from("missing value for --db"))?,
                );
            }
            "--help" | "-h" => return Err(String::from("__help__")),
            other => return Err(format!("unknown argument: {other}")),
        }
    }

    Ok(db_path)
}

fn print_help() {
    println!("nime-registry commands:");
    println!("  init-db [--db server/registry/nime-registry.db]");
    println!(
        "  add --name portal --group core --target-ip 10.42.0.1 --port 8080 --proto http --title Portal --owner arthur [--description ...] [--tags infra,search]"
    );
    println!(
        "  update --name portal [--group core] [--target-ip 10.42.0.2] [--port 3000] [--proto http] [--title ...] [--description ...] [--owner ...] [--tags ...]"
    );
    println!("  list [--db ...]");
    println!("  get --name portal [--db ...]");
    println!("  remove --name portal [--db ...]");
    println!("  export-dns [--db ...]");
    println!("  bootstrap --portal-ip 10.42.0.1 [--portal-port 8080] [--owner admin] [--db ...]");
    println!("  serve [--db ...] [--bind 127.0.0.1:4172]");
}

fn default_db_path() -> PathBuf {
    PathBuf::from(DEFAULT_DB_PATH)
}

fn open_db(path: &Path) -> Result<Connection, String> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .map_err(|error| format!("failed to create {}: {error}", parent.display()))?;
    }
    Connection::open(path).map_err(|error| format!("failed to open {}: {error}", path.display()))
}

fn init_schema(conn: &Connection) -> Result<(), String> {
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS services (
            name TEXT PRIMARY KEY,
            group_name TEXT NOT NULL DEFAULT 'apps',
            target_ip TEXT NOT NULL,
            port INTEGER NOT NULL,
            protocol TEXT NOT NULL,
            title TEXT NOT NULL,
            description TEXT NOT NULL,
            owner TEXT NOT NULL,
            tags TEXT NOT NULL,
            created_at INTEGER NOT NULL
        );",
    )
    .map_err(|error| format!("failed to initialize schema: {error}"))?;
    ensure_group_column(conn)
}

fn ensure_group_column(conn: &Connection) -> Result<(), String> {
    conn.execute(
        "ALTER TABLE services ADD COLUMN group_name TEXT NOT NULL DEFAULT 'apps'",
        [],
    )
    .or_else(|error| {
        let message = error.to_string().to_ascii_lowercase();
        if message.contains("duplicate column") || message.contains("already exists") {
            Ok(0)
        } else {
            Err(error)
        }
    })
    .map(|_| ())
    .map_err(|error| format!("failed to ensure group_name column: {error}"))
}

fn insert_service(conn: &Connection, service: &NewService) -> Result<(), String> {
    let created_at = now_unix()?;
    conn.execute(
        "INSERT INTO services
        (name, group_name, target_ip, port, protocol, title, description, owner, tags, created_at)
        VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
        params![
            service.name,
            service.group_name,
            service.target_ip.to_string(),
            service.port,
            service.protocol,
            service.title,
            service.description,
            service.owner,
            join_tags(&service.tags),
            created_at,
        ],
    )
    .map_err(|error| format!("failed to insert service `{}`: {error}", service.name))?;
    Ok(())
}

fn upsert_service(conn: &Connection, service: &NewService) -> Result<(), String> {
    let created_at = now_unix()?;
    conn.execute(
        "INSERT INTO services
        (name, group_name, target_ip, port, protocol, title, description, owner, tags, created_at)
        VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)
        ON CONFLICT(name) DO UPDATE SET
            group_name = excluded.group_name,
            target_ip = excluded.target_ip,
            port = excluded.port,
            protocol = excluded.protocol,
            title = excluded.title,
            description = excluded.description,
            owner = excluded.owner,
            tags = excluded.tags",
        params![
            service.name,
            service.group_name,
            service.target_ip.to_string(),
            service.port,
            service.protocol,
            service.title,
            service.description,
            service.owner,
            join_tags(&service.tags),
            created_at,
        ],
    )
    .map_err(|error| format!("failed to upsert service `{}`: {error}", service.name))?;
    Ok(())
}

fn list_services(conn: &Connection) -> Result<Vec<Service>, String> {
    let mut stmt = conn
        .prepare(
            "SELECT name, group_name, target_ip, port, protocol, title, description, owner, tags, created_at
            FROM services
            ORDER BY group_name ASC, name ASC",
        )
        .map_err(|error| format!("failed to prepare list query: {error}"))?;
    let rows = stmt
        .query_map([], map_service_row)
        .map_err(|error| format!("failed to query services: {error}"))?;

    let mut services = Vec::new();
    for row in rows {
        services.push(row.map_err(|error| format!("failed to read service row: {error}"))?);
    }
    Ok(services)
}

fn get_service(conn: &Connection, name: &str) -> Result<Option<Service>, String> {
    conn.query_row(
        "SELECT name, group_name, target_ip, port, protocol, title, description, owner, tags, created_at
        FROM services
        WHERE name = ?1",
        params![name],
        map_service_row,
    )
    .optional()
    .map_err(|error| format!("failed to load service `{name}`: {error}"))
}

fn delete_service(conn: &Connection, name: &str) -> Result<usize, String> {
    conn.execute("DELETE FROM services WHERE name = ?1", params![name])
        .map_err(|error| format!("failed to delete service `{name}`: {error}"))
}

fn update_service(
    conn: &Connection,
    name: &str,
    update: &UpdateService,
) -> Result<Service, String> {
    let current = get_service(conn, name)?.ok_or_else(|| format!("service `{name}` not found"))?;
    let merged = NewService {
        name: current.name.clone(),
        group_name: update
            .group_name
            .clone()
            .unwrap_or_else(|| current.group_name.clone()),
        target_ip: update.target_ip.unwrap_or(current.target_ip),
        port: update.port.unwrap_or(current.port),
        protocol: update
            .protocol
            .clone()
            .unwrap_or_else(|| current.protocol.clone()),
        title: update
            .title
            .clone()
            .unwrap_or_else(|| current.title.clone()),
        description: update
            .description
            .clone()
            .unwrap_or_else(|| current.description.clone()),
        owner: update
            .owner
            .clone()
            .unwrap_or_else(|| current.owner.clone()),
        tags: update.tags.clone().unwrap_or_else(|| current.tags.clone()),
    };

    upsert_service(conn, &merged)?;
    get_service(conn, name)?.ok_or_else(|| format!("service `{name}` disappeared after update"))
}

fn search_services(conn: &Connection, query: &str) -> Result<Vec<Service>, String> {
    let needle = query.trim().to_ascii_lowercase();
    if needle.is_empty() {
        return list_services(conn);
    }

    let services = list_services(conn)?;
    Ok(services
        .into_iter()
        .filter(|service| {
            service.name.contains(&needle)
                || service.group_name.contains(&needle)
                || service.title.to_ascii_lowercase().contains(&needle)
                || service.description.to_ascii_lowercase().contains(&needle)
                || service.owner.to_ascii_lowercase().contains(&needle)
                || service.tags.iter().any(|tag| tag.contains(&needle))
        })
        .collect())
}

fn map_service_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<Service> {
    let target_ip: String = row.get(2)?;
    Ok(Service {
        name: row.get(0)?,
        group_name: row.get(1)?,
        target_ip: target_ip.parse().map_err(|_| {
            rusqlite::Error::FromSqlConversionFailure(
                2,
                rusqlite::types::Type::Text,
                Box::<dyn std::error::Error + Send + Sync>::from("invalid ipv4"),
            )
        })?,
        port: row.get(3)?,
        protocol: row.get(4)?,
        title: row.get(5)?,
        description: row.get(6)?,
        owner: row.get(7)?,
        tags: parse_tags(&row.get::<_, String>(8)?),
        created_at: row.get(9)?,
    })
}

fn print_service(service: &Service) {
    println!("name: {}", service.name);
    println!("group: {}", service.group_name);
    println!("title: {}", service.title);
    println!(
        "target: {}://{}:{}",
        service.protocol, service.target_ip, service.port
    );
    println!("owner: {}", service.owner);
    println!(
        "tags: {}",
        if service.tags.is_empty() {
            String::from("-")
        } else {
            service.tags.join(",")
        }
    );
    println!(
        "description: {}",
        if service.description.is_empty() {
            "-"
        } else {
            &service.description
        }
    );
    println!("created_at: {}", service.created_at);
}

fn normalize_name(value: &str) -> Result<String, String> {
    let normalized = value.trim().trim_end_matches('.').to_ascii_lowercase();
    validate_non_empty(&normalized, "name")?;
    if normalized.len() > 63 {
        return Err(String::from("name must be 63 chars or shorter"));
    }
    if !normalized
        .chars()
        .all(|ch| ch.is_ascii_lowercase() || ch.is_ascii_digit() || ch == '-')
    {
        return Err(String::from(
            "name must contain only lowercase letters, digits or hyphen",
        ));
    }
    Ok(normalized)
}

fn validate_non_empty(value: &str, field: &str) -> Result<(), String> {
    if value.trim().is_empty() {
        Err(format!("{field} must not be empty"))
    } else {
        Ok(())
    }
}

fn parse_tags(value: &str) -> Vec<String> {
    value
        .split(',')
        .map(str::trim)
        .filter(|tag| !tag.is_empty())
        .map(|tag| tag.to_ascii_lowercase())
        .collect()
}

fn join_tags(tags: &[String]) -> String {
    tags.join(",")
}

fn now_unix() -> Result<u64, String> {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .map_err(|error| format!("system time error: {error}"))
}

fn run_http_api(db_path: &Path, bind: SocketAddr) -> Result<(), String> {
    let listener = TcpListener::bind(bind)
        .map_err(|error| format!("failed to bind registry API on {bind}: {error}"))?;
    println!(
        "nime-registry API listening on {} using db {}",
        bind,
        db_path.display()
    );

    for stream in listener.incoming() {
        match stream {
            Ok(stream) => {
                if let Err(error) = handle_http_connection(stream, db_path) {
                    eprintln!("nime-registry api warning: {error}");
                }
            }
            Err(error) => eprintln!("nime-registry api accept error: {error}"),
        }
    }

    Ok(())
}

fn handle_http_connection(mut stream: TcpStream, db_path: &Path) -> Result<(), String> {
    stream
        .set_read_timeout(Some(std::time::Duration::from_secs(2)))
        .map_err(|error| format!("failed to set read timeout: {error}"))?;

    let mut buffer = [0_u8; 8192];
    let size = stream
        .read(&mut buffer)
        .map_err(|error| format!("failed to read request: {error}"))?;
    if size == 0 {
        return Ok(());
    }

    let request = String::from_utf8_lossy(&buffer[..size]);
    let first_line = request
        .lines()
        .next()
        .ok_or_else(|| String::from("empty HTTP request"))?;
    let mut parts = first_line.split_whitespace();
    let method = parts.next().unwrap_or("");
    let target = parts.next().unwrap_or("");

    if method != "GET" {
        return write_http_response(
            &mut stream,
            405,
            "text/plain; charset=utf-8",
            "method not allowed\n",
        );
    }

    let conn = open_db(db_path)?;
    init_schema(&conn)?;
    let (path, query) = split_path_query(target);

    match path {
        "/health" => write_http_response(&mut stream, 200, "text/plain; charset=utf-8", "ok\n"),
        "/services" => {
            let services = if let Some(query) = query_param(query, "q") {
                search_services(&conn, &query)?
            } else {
                list_services(&conn)?
            };
            let body = services_to_json(&services).to_string();
            write_http_response(&mut stream, 200, "application/json", &body)
        }
        _ if path.starts_with("/services/") => {
            let name = normalize_name(path.trim_start_matches("/services/"))?;
            let body = if let Some(service) = get_service(&conn, &name)? {
                service_to_json(&service).to_string()
            } else {
                json!({ "error": format!("service `{name}` not found") }).to_string()
            };
            let status = if body.contains("\"error\"") { 404 } else { 200 };
            write_http_response(&mut stream, status, "application/json", &body)
        }
        _ => write_http_response(
            &mut stream,
            404,
            "application/json",
            &json!({
                "error": "not found",
                "routes": ["/health", "/services", "/services/<name>"]
            })
            .to_string(),
        ),
    }
}

fn write_http_response(
    stream: &mut TcpStream,
    status: u16,
    content_type: &str,
    body: &str,
) -> Result<(), String> {
    let reason = match status {
        200 => "OK",
        404 => "Not Found",
        405 => "Method Not Allowed",
        _ => "Error",
    };
    let response = format!(
        "HTTP/1.1 {status} {reason}\r\nContent-Type: {content_type}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
        body.len()
    );
    stream
        .write_all(response.as_bytes())
        .map_err(|error| format!("failed to write HTTP response: {error}"))
}

fn split_path_query(target: &str) -> (&str, &str) {
    target.split_once('?').unwrap_or((target, ""))
}

fn query_param(query: &str, key: &str) -> Option<String> {
    query.split('&').find_map(|pair| {
        let (candidate_key, value) = pair.split_once('=')?;
        if candidate_key == key {
            Some(url_decode(value))
        } else {
            None
        }
    })
}

fn url_decode(value: &str) -> String {
    let bytes = value.as_bytes();
    let mut decoded = String::new();
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

fn service_to_json(service: &Service) -> Value {
    json!({
        "name": service.name,
        "group": service.group_name,
        "target_ip": service.target_ip.to_string(),
        "port": service.port,
        "protocol": service.protocol,
        "title": service.title,
        "description": service.description,
        "owner": service.owner,
        "tags": service.tags,
        "created_at": service.created_at,
    })
}

fn services_to_json(services: &[Service]) -> Value {
    Value::Array(services.iter().map(service_to_json).collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_db_path(name: &str) -> PathBuf {
        std::env::temp_dir().join(format!(
            "nime-registry-{}-{}-{}.db",
            name,
            process::id(),
            now_unix().unwrap()
        ))
    }

    #[test]
    fn parses_add_command() {
        let command = parse_cli(vec![
            String::from("add"),
            String::from("--name"),
            String::from("portal"),
            String::from("--target-ip"),
            String::from("10.60.0.10"),
            String::from("--owner"),
            String::from("arthur"),
        ])
        .unwrap();

        match command {
            Command::Add {
                name,
                group_name,
                target_ip,
                port,
                protocol,
                owner,
                ..
            } => {
                assert_eq!(name, "portal");
                assert_eq!(group_name, "apps");
                assert_eq!(target_ip, Ipv4Addr::new(10, 60, 0, 10));
                assert_eq!(port, 80);
                assert_eq!(protocol, "http");
                assert_eq!(owner, "arthur");
            }
            _ => panic!("expected add command"),
        }
    }

    #[test]
    fn inserts_and_lists_service() {
        let db_path = temp_db_path("list");
        let conn = open_db(&db_path).unwrap();
        init_schema(&conn).unwrap();

        insert_service(
            &conn,
            &NewService {
                name: String::from("portal"),
                group_name: String::from("core"),
                target_ip: Ipv4Addr::new(10, 60, 0, 10),
                port: 80,
                protocol: String::from("http"),
                title: String::from("Portal"),
                description: String::from("Shared start page"),
                owner: String::from("arthur"),
                tags: vec![String::from("infra"), String::from("search")],
            },
        )
        .unwrap();

        let services = list_services(&conn).unwrap();
        assert_eq!(services.len(), 1);
        assert_eq!(services[0].name, "portal");
        assert_eq!(services[0].group_name, "core");
        assert_eq!(services[0].tags, vec!["infra", "search"]);

        let _ = fs::remove_file(db_path);
    }

    #[test]
    fn exports_dns_line_shape() {
        let service = Service {
            name: String::from("wiki"),
            group_name: String::from("docs"),
            target_ip: Ipv4Addr::new(10, 60, 0, 12),
            port: 8080,
            protocol: String::from("http"),
            title: String::from("Wiki"),
            description: String::new(),
            owner: String::from("arthur"),
            tags: vec![],
            created_at: 1,
        };

        assert_eq!(
            format!(
                "{}.{} {}",
                service.name, service.group_name, service.target_ip
            ),
            "wiki.docs 10.60.0.12"
        );
    }

    #[test]
    fn parses_bootstrap_command() {
        let command = parse_cli(vec![
            String::from("bootstrap"),
            String::from("--portal-ip"),
            String::from("10.60.0.10"),
            String::from("--owner"),
            String::from("arthur"),
        ])
        .unwrap();

        match command {
            Command::Bootstrap {
                portal_ip,
                portal_port,
                owner,
                ..
            } => {
                assert_eq!(portal_ip, Ipv4Addr::new(10, 60, 0, 10));
                assert_eq!(portal_port, 8080);
                assert_eq!(owner, "arthur");
            }
            _ => panic!("expected bootstrap command"),
        }
    }

    #[test]
    fn parses_serve_command() {
        let command = parse_cli(vec![
            String::from("serve"),
            String::from("--bind"),
            String::from("0.0.0.0:4172"),
        ])
        .unwrap();

        match command {
            Command::Serve { bind, .. } => assert_eq!(bind, "0.0.0.0:4172".parse().unwrap()),
            _ => panic!("expected serve command"),
        }
    }

    #[test]
    fn parses_update_command() {
        let command = parse_cli(vec![
            String::from("update"),
            String::from("--name"),
            String::from("portal"),
            String::from("--port"),
            String::from("3000"),
            String::from("--owner"),
            String::from("arthur"),
        ])
        .unwrap();

        match command {
            Command::Update {
                name,
                group_name,
                port,
                owner,
                ..
            } => {
                assert_eq!(name, "portal");
                assert!(group_name.is_none());
                assert_eq!(port, Some(3000));
                assert_eq!(owner.as_deref(), Some("arthur"));
            }
            _ => panic!("expected update command"),
        }
    }

    #[test]
    fn search_matches_title_and_tags() {
        let db_path = temp_db_path("search");
        let conn = open_db(&db_path).unwrap();
        init_schema(&conn).unwrap();

        insert_service(
            &conn,
            &NewService {
                name: String::from("portal"),
                group_name: String::from("core"),
                target_ip: Ipv4Addr::new(10, 42, 0, 1),
                port: 8080,
                protocol: String::from("http"),
                title: String::from("Nime Portal"),
                description: String::from("Shared start page"),
                owner: String::from("arthur"),
                tags: vec![String::from("infra"), String::from("search")],
            },
        )
        .unwrap();

        let services = search_services(&conn, "search").unwrap();
        assert_eq!(services.len(), 1);
        assert_eq!(services[0].name, "portal");

        let _ = fs::remove_file(db_path);
    }

    #[test]
    fn encodes_services_as_json() {
        let service = Service {
            name: String::from("portal"),
            group_name: String::from("core"),
            target_ip: Ipv4Addr::new(10, 42, 0, 1),
            port: 8080,
            protocol: String::from("http"),
            title: String::from("Portal"),
            description: String::from("Shared start page"),
            owner: String::from("arthur"),
            tags: vec![String::from("infra")],
            created_at: 1,
        };

        let json = service_to_json(&service);
        assert_eq!(json["name"], "portal");
        assert_eq!(json["group"], "core");
        assert_eq!(json["target_ip"], "10.42.0.1");
        assert_eq!(json["port"], 8080);
    }

    #[test]
    fn updates_existing_service() {
        let db_path = temp_db_path("update");
        let conn = open_db(&db_path).unwrap();
        init_schema(&conn).unwrap();

        insert_service(
            &conn,
            &NewService {
                name: String::from("portal"),
                group_name: String::from("core"),
                target_ip: Ipv4Addr::new(10, 42, 0, 1),
                port: 8080,
                protocol: String::from("http"),
                title: String::from("Portal"),
                description: String::from("Shared start page"),
                owner: String::from("arthur"),
                tags: vec![String::from("infra")],
            },
        )
        .unwrap();

        let service = update_service(
            &conn,
            "portal",
            &UpdateService {
                group_name: Some(String::from("apps")),
                target_ip: Some(Ipv4Addr::new(10, 42, 0, 2)),
                port: Some(3000),
                protocol: None,
                title: None,
                description: Some(String::from("Updated")),
                owner: None,
                tags: Some(vec![String::from("infra"), String::from("search")]),
            },
        )
        .unwrap();

        assert_eq!(service.target_ip, Ipv4Addr::new(10, 42, 0, 2));
        assert_eq!(service.group_name, "apps");
        assert_eq!(service.port, 3000);
        assert_eq!(service.description, "Updated");
        assert_eq!(service.tags, vec!["infra", "search"]);

        let _ = fs::remove_file(db_path);
    }
}
