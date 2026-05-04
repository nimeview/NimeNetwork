use std::{
    env, fmt, fs,
    io::{Read, Write},
    net::{Ipv4Addr, SocketAddr, TcpStream, UdpSocket},
    path::{Path, PathBuf},
    process::{self, Command as ProcessCommand},
    time::Duration,
};

use serde_json::Value;

fn main() {
    if let Err(error) = run(env::args().skip(1).collect()) {
        eprintln!("nime-client error: {error}");
        process::exit(1);
    }
}

fn run(args: Vec<String>) -> Result<(), String> {
    let command = parse_cli(args)?;

    match command {
        Command::Init {
            config_path,
            network,
            dns_server,
            dns_zone,
            innernet_network,
            innernet_binary,
            wg_userspace_implementation,
        } => {
            let config = ClientConfig {
                network,
                dns_server,
                dns_zone,
                innernet_network,
                innernet_binary,
                wg_userspace_implementation,
            };
            write_config(&config_path, &config)?;
            println!("initialized client config at {}", config_path.display());
            println!("{config}");
        }
        Command::Status { config_path } => {
            let config = read_config(&config_path)?;
            print_status(&config_path, &config);
        }
        Command::ShowConfig { config_path } => {
            let config = read_config(&config_path)?;
            print_config(&config_path, &config);
        }
        Command::Join {
            config_path,
            invitation,
            name,
        } => {
            let config = read_config(&config_path)?;
            run_innernet_install(&config, &invitation, name.as_deref())?;
        }
        Command::Peers { config_path, tree } => {
            let config = read_config(&config_path)?;
            run_innernet_list(&config, tree)?;
        }
        Command::Fetch { config_path } => {
            let config = read_config(&config_path)?;
            run_innernet_interface_command(&config, "fetch", &[])?;
        }
        Command::Up { config_path } => {
            let config = read_config(&config_path)?;
            run_innernet_interface_command(&config, "up", &[])?;
        }
        Command::Disconnect {
            config_path,
            dns_format,
            dns_output,
            keep_dns,
        } => {
            let config = read_config(&config_path)?;
            run_innernet_interface_command(&config, "down", &[])?;
            if !keep_dns {
                remove_dns_config(&config, dns_format, dns_output.as_deref())?;
            }
            println!("disconnect complete");
        }
        Command::Doctor { config_path } => {
            let config = read_config(&config_path)?;
            run_doctor(&config_path, &config);
        }
        Command::DnsPrint {
            config_path,
            format,
        } => {
            let config = read_config(&config_path)?;
            print_dns_config(&config, format);
        }
        Command::DnsInstall {
            config_path,
            format,
            output_path,
        } => {
            let config = read_config(&config_path)?;
            install_dns_config(&config, format, output_path.as_deref())?;
        }
        Command::Setup {
            config_path,
            invitation,
            name,
            dns_format,
            dns_output,
            skip_dns,
        } => {
            let config = read_config(&config_path)?;
            run_innernet_install(&config, &invitation, name.as_deref())?;
            run_innernet_interface_command(&config, "up", &[])?;
            run_innernet_interface_command(&config, "fetch", &[])?;
            if !skip_dns {
                install_dns_config(&config, dns_format, dns_output.as_deref())?;
            }
            println!(
                "setup complete. next step: open http://portal.{} in your browser once the service exists",
                config.dns_zone
            );
        }
        Command::Reconnect {
            config_path,
            dns_format,
            dns_output,
            skip_dns,
        } => {
            let config = read_config(&config_path)?;
            run_innernet_interface_command(&config, "up", &[])?;
            run_innernet_interface_command(&config, "fetch", &[])?;
            if !skip_dns {
                install_dns_config(&config, dns_format, dns_output.as_deref())?;
            }
            println!("reconnect complete");
        }
        Command::AutostartPrint {
            config_path,
            format,
        } => {
            let config = read_config(&config_path)?;
            print_autostart_config(&config, &config_path, format);
        }
        Command::AutostartInstall {
            config_path,
            format,
            output_path,
        } => {
            let config = read_config(&config_path)?;
            install_autostart_config(&config, &config_path, format, output_path.as_deref())?;
        }
        Command::Resolve { config_path, name } => {
            let config = read_config(&config_path)?;
            let fqdn = absolute_name(&name, &config.dns_zone);
            let records = resolve_a_record(config.dns_server, &fqdn)?;
            if records.is_empty() {
                println!("{fqdn} -> no A records");
            } else {
                for ip in records {
                    println!("{fqdn} -> {ip}");
                }
            }
        }
        Command::Health {
            config_path,
            service,
            peer,
            port,
        } => {
            let config = read_config(&config_path)?;
            run_health(&config, service.as_deref(), peer.as_deref(), port)?;
        }
        Command::Open {
            config_path,
            service,
            port,
            https,
        } => {
            let config = read_config(&config_path)?;
            open_service(&config, service.as_deref().unwrap_or("portal"), port, https)?;
        }
        Command::Services {
            config_path,
            query,
            json,
        } => {
            let config = read_config(&config_path)?;
            print_services(&config, query.as_deref(), json)?;
        }
        Command::Help => print_help(),
    }

    Ok(())
}

#[derive(Debug)]
enum Command {
    Init {
        config_path: PathBuf,
        network: String,
        dns_server: SocketAddr,
        dns_zone: String,
        innernet_network: String,
        innernet_binary: String,
        wg_userspace_implementation: Option<String>,
    },
    Status {
        config_path: PathBuf,
    },
    ShowConfig {
        config_path: PathBuf,
    },
    Join {
        config_path: PathBuf,
        invitation: PathBuf,
        name: Option<String>,
    },
    Peers {
        config_path: PathBuf,
        tree: bool,
    },
    Fetch {
        config_path: PathBuf,
    },
    Up {
        config_path: PathBuf,
    },
    Disconnect {
        config_path: PathBuf,
        dns_format: DnsFormat,
        dns_output: Option<PathBuf>,
        keep_dns: bool,
    },
    Doctor {
        config_path: PathBuf,
    },
    DnsPrint {
        config_path: PathBuf,
        format: DnsFormat,
    },
    DnsInstall {
        config_path: PathBuf,
        format: DnsFormat,
        output_path: Option<PathBuf>,
    },
    Setup {
        config_path: PathBuf,
        invitation: PathBuf,
        name: Option<String>,
        dns_format: DnsFormat,
        dns_output: Option<PathBuf>,
        skip_dns: bool,
    },
    Reconnect {
        config_path: PathBuf,
        dns_format: DnsFormat,
        dns_output: Option<PathBuf>,
        skip_dns: bool,
    },
    AutostartPrint {
        config_path: PathBuf,
        format: AutostartFormat,
    },
    AutostartInstall {
        config_path: PathBuf,
        format: AutostartFormat,
        output_path: Option<PathBuf>,
    },
    Resolve {
        config_path: PathBuf,
        name: String,
    },
    Health {
        config_path: PathBuf,
        service: Option<String>,
        peer: Option<String>,
        port: Option<u16>,
    },
    Open {
        config_path: PathBuf,
        service: Option<String>,
        port: Option<u16>,
        https: bool,
    },
    Services {
        config_path: PathBuf,
        query: Option<String>,
        json: bool,
    },
    Help,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum DnsFormat {
    Resolver,
    Resolved,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum AutostartFormat {
    Launchd,
    SystemdUser,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ClientConfig {
    network: String,
    dns_server: SocketAddr,
    dns_zone: String,
    innernet_network: String,
    innernet_binary: String,
    wg_userspace_implementation: Option<String>,
}

impl ClientConfig {
    fn to_file_contents(&self) -> String {
        format!(
            "network = {}\ninnernet_network = {}\ninnernet_binary = {}\nwg_userspace_implementation = {}\ndns_server = {}\ndns_zone = {}\n",
            self.network,
            self.innernet_network,
            self.innernet_binary,
            self.wg_userspace_implementation.as_deref().unwrap_or(""),
            self.dns_server,
            self.dns_zone
        )
    }
}

fn parse_cli(args: Vec<String>) -> Result<Command, String> {
    let mut args = args.into_iter();
    let Some(command) = args.next() else {
        return Ok(Command::Help);
    };

    match command.as_str() {
        "init" => parse_init(args.collect()),
        "status" => parse_single_path_command(args.collect(), CommandKind::Status),
        "show-config" => parse_single_path_command(args.collect(), CommandKind::ShowConfig),
        "join" => parse_join(args.collect()),
        "peers" => parse_peers(args.collect()),
        "fetch" => parse_single_path_command(args.collect(), CommandKind::Fetch),
        "up" => parse_single_path_command(args.collect(), CommandKind::Up),
        "disconnect" => parse_disconnect(args.collect()),
        "doctor" => parse_single_path_command(args.collect(), CommandKind::Doctor),
        "dns-print" => parse_dns_print(args.collect()),
        "dns-install" => parse_dns_install(args.collect()),
        "setup" => parse_setup(args.collect()),
        "reconnect" => parse_reconnect(args.collect()),
        "autostart-print" => parse_autostart_print(args.collect()),
        "autostart-install" => parse_autostart_install(args.collect()),
        "resolve" => parse_resolve(args.collect()),
        "health" => parse_health(args.collect()),
        "open" => parse_open(args.collect()),
        "services" => parse_services(args.collect()),
        "--help" | "-h" | "help" => Ok(Command::Help),
        other => Err(format!("unknown command: {other}")),
    }
}

enum CommandKind {
    Status,
    ShowConfig,
    Fetch,
    Up,
    Doctor,
}

fn parse_single_path_command(args: Vec<String>, kind: CommandKind) -> Result<Command, String> {
    let mut config_path = default_config_path();
    let mut args = args.into_iter();

    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--config" => {
                config_path = PathBuf::from(
                    args.next()
                        .ok_or_else(|| String::from("missing value for --config"))?,
                );
            }
            "--help" | "-h" => return Ok(Command::Help),
            other => return Err(format!("unknown argument: {other}")),
        }
    }

    Ok(match kind {
        CommandKind::Status => Command::Status { config_path },
        CommandKind::ShowConfig => Command::ShowConfig { config_path },
        CommandKind::Fetch => Command::Fetch { config_path },
        CommandKind::Up => Command::Up { config_path },
        CommandKind::Doctor => Command::Doctor { config_path },
    })
}

fn parse_dns_print(args: Vec<String>) -> Result<Command, String> {
    let mut config_path = default_config_path();
    let mut format = DnsFormat::Resolver;
    let mut args = args.into_iter();

    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--config" => {
                config_path = PathBuf::from(
                    args.next()
                        .ok_or_else(|| String::from("missing value for --config"))?,
                );
            }
            "--format" => {
                let value = args
                    .next()
                    .ok_or_else(|| String::from("missing value for --format"))?;
                format = match value.as_str() {
                    "resolver" => DnsFormat::Resolver,
                    "resolved" => DnsFormat::Resolved,
                    _ => return Err(format!("unsupported dns format: {value}")),
                };
            }
            "--help" | "-h" => return Ok(Command::Help),
            other => return Err(format!("unknown argument: {other}")),
        }
    }

    Ok(Command::DnsPrint {
        config_path,
        format,
    })
}

fn parse_dns_install(args: Vec<String>) -> Result<Command, String> {
    let (config_path, format, output_path) = parse_dns_command_args(args)?;
    Ok(Command::DnsInstall {
        config_path,
        format,
        output_path,
    })
}

fn parse_setup(args: Vec<String>) -> Result<Command, String> {
    let mut config_path = default_config_path();
    let mut invitation = None;
    let mut name = None;
    let mut dns_format = DnsFormat::Resolver;
    let mut dns_output = None;
    let mut skip_dns = false;
    let mut args = args.into_iter();

    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--config" => {
                config_path = PathBuf::from(
                    args.next()
                        .ok_or_else(|| String::from("missing value for --config"))?,
                );
            }
            "--invite" => {
                invitation = Some(PathBuf::from(
                    args.next()
                        .ok_or_else(|| String::from("missing value for --invite"))?,
                ));
            }
            "--name" => {
                name = Some(
                    args.next()
                        .ok_or_else(|| String::from("missing value for --name"))?,
                );
            }
            "--dns-format" => {
                let value = args
                    .next()
                    .ok_or_else(|| String::from("missing value for --dns-format"))?;
                dns_format = parse_dns_format(&value)?;
            }
            "--dns-output" => {
                dns_output =
                    Some(PathBuf::from(args.next().ok_or_else(|| {
                        String::from("missing value for --dns-output")
                    })?));
            }
            "--skip-dns" => skip_dns = true,
            "--help" | "-h" => return Ok(Command::Help),
            other => return Err(format!("unknown argument: {other}")),
        }
    }

    Ok(Command::Setup {
        config_path,
        invitation: invitation.ok_or_else(|| String::from("missing required --invite"))?,
        name,
        dns_format,
        dns_output,
        skip_dns,
    })
}

fn parse_reconnect(args: Vec<String>) -> Result<Command, String> {
    let mut config_path = default_config_path();
    let mut dns_format = DnsFormat::Resolver;
    let mut dns_output = None;
    let mut skip_dns = false;
    let mut args = args.into_iter();

    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--config" => {
                config_path = PathBuf::from(
                    args.next()
                        .ok_or_else(|| String::from("missing value for --config"))?,
                );
            }
            "--dns-format" => {
                let value = args
                    .next()
                    .ok_or_else(|| String::from("missing value for --dns-format"))?;
                dns_format = parse_dns_format(&value)?;
            }
            "--dns-output" => {
                dns_output =
                    Some(PathBuf::from(args.next().ok_or_else(|| {
                        String::from("missing value for --dns-output")
                    })?));
            }
            "--skip-dns" => skip_dns = true,
            "--help" | "-h" => return Ok(Command::Help),
            other => return Err(format!("unknown argument: {other}")),
        }
    }

    Ok(Command::Reconnect {
        config_path,
        dns_format,
        dns_output,
        skip_dns,
    })
}

fn parse_disconnect(args: Vec<String>) -> Result<Command, String> {
    let mut config_path = default_config_path();
    let mut dns_format = DnsFormat::Resolver;
    let mut dns_output = None;
    let mut keep_dns = false;
    let mut args = args.into_iter();

    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--config" => {
                config_path = PathBuf::from(
                    args.next()
                        .ok_or_else(|| String::from("missing value for --config"))?,
                );
            }
            "--dns-format" => {
                let value = args
                    .next()
                    .ok_or_else(|| String::from("missing value for --dns-format"))?;
                dns_format = parse_dns_format(&value)?;
            }
            "--dns-output" => {
                dns_output =
                    Some(PathBuf::from(args.next().ok_or_else(|| {
                        String::from("missing value for --dns-output")
                    })?));
            }
            "--keep-dns" => keep_dns = true,
            "--help" | "-h" => return Ok(Command::Help),
            other => return Err(format!("unknown argument: {other}")),
        }
    }

    Ok(Command::Disconnect {
        config_path,
        dns_format,
        dns_output,
        keep_dns,
    })
}

fn parse_autostart_print(args: Vec<String>) -> Result<Command, String> {
    let (config_path, format, _) = parse_autostart_args(args)?;
    Ok(Command::AutostartPrint {
        config_path,
        format,
    })
}

fn parse_autostart_install(args: Vec<String>) -> Result<Command, String> {
    let (config_path, format, output_path) = parse_autostart_args(args)?;
    Ok(Command::AutostartInstall {
        config_path,
        format,
        output_path,
    })
}

fn parse_resolve(args: Vec<String>) -> Result<Command, String> {
    let mut config_path = default_config_path();
    let mut name = None;
    let mut args = args.into_iter();

    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--config" => {
                config_path = PathBuf::from(
                    args.next()
                        .ok_or_else(|| String::from("missing value for --config"))?,
                );
            }
            "--name" => {
                name = Some(
                    args.next()
                        .ok_or_else(|| String::from("missing value for --name"))?,
                );
            }
            "--help" | "-h" => return Ok(Command::Help),
            other if name.is_none() && !other.starts_with("--") => name = Some(other.to_owned()),
            other => return Err(format!("unknown argument: {other}")),
        }
    }

    Ok(Command::Resolve {
        config_path,
        name: name.ok_or_else(|| String::from("missing service or peer name"))?,
    })
}

fn parse_health(args: Vec<String>) -> Result<Command, String> {
    let mut config_path = default_config_path();
    let mut service = None;
    let mut peer = None;
    let mut port = None;
    let mut args = args.into_iter();

    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--config" => {
                config_path = PathBuf::from(
                    args.next()
                        .ok_or_else(|| String::from("missing value for --config"))?,
                );
            }
            "--service" => {
                service = Some(
                    args.next()
                        .ok_or_else(|| String::from("missing value for --service"))?,
                );
            }
            "--peer" => {
                peer = Some(
                    args.next()
                        .ok_or_else(|| String::from("missing value for --peer"))?,
                );
            }
            "--port" => {
                let value = args
                    .next()
                    .ok_or_else(|| String::from("missing value for --port"))?;
                port = Some(
                    value
                        .parse()
                        .map_err(|_| format!("invalid port value: {value}"))?,
                );
            }
            "--help" | "-h" => return Ok(Command::Help),
            other if service.is_none() && peer.is_none() && !other.starts_with("--") => {
                service = Some(other.to_owned())
            }
            other => return Err(format!("unknown argument: {other}")),
        }
    }

    if service.is_some() && peer.is_some() {
        return Err(String::from("use either --service or --peer, not both"));
    }

    Ok(Command::Health {
        config_path,
        service,
        peer,
        port,
    })
}

fn parse_open(args: Vec<String>) -> Result<Command, String> {
    let mut config_path = default_config_path();
    let mut service = None;
    let mut port = None;
    let mut https = false;
    let mut args = args.into_iter();

    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--config" => {
                config_path = PathBuf::from(
                    args.next()
                        .ok_or_else(|| String::from("missing value for --config"))?,
                );
            }
            "--service" => {
                service = Some(
                    args.next()
                        .ok_or_else(|| String::from("missing value for --service"))?,
                );
            }
            "--port" => {
                let value = args
                    .next()
                    .ok_or_else(|| String::from("missing value for --port"))?;
                port = Some(
                    value
                        .parse()
                        .map_err(|_| format!("invalid port value: {value}"))?,
                );
            }
            "--https" => https = true,
            "--help" | "-h" => return Ok(Command::Help),
            other if service.is_none() && !other.starts_with("--") => {
                service = Some(other.to_owned())
            }
            other => return Err(format!("unknown argument: {other}")),
        }
    }

    Ok(Command::Open {
        config_path,
        service,
        port,
        https,
    })
}

fn parse_services(args: Vec<String>) -> Result<Command, String> {
    let mut config_path = default_config_path();
    let mut query = None;
    let mut json = false;
    let mut args = args.into_iter();

    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--config" => {
                config_path = PathBuf::from(
                    args.next()
                        .ok_or_else(|| String::from("missing value for --config"))?,
                );
            }
            "--query" | "-q" => {
                query = Some(
                    args.next()
                        .ok_or_else(|| String::from("missing value for --query"))?,
                );
            }
            "--json" => json = true,
            "--help" | "-h" => return Ok(Command::Help),
            other if query.is_none() && !other.starts_with("--") => query = Some(other.to_owned()),
            other => return Err(format!("unknown argument: {other}")),
        }
    }

    Ok(Command::Services {
        config_path,
        query,
        json,
    })
}

fn parse_autostart_args(
    args: Vec<String>,
) -> Result<(PathBuf, AutostartFormat, Option<PathBuf>), String> {
    let mut config_path = default_config_path();
    let mut format = AutostartFormat::Launchd;
    let mut output_path = None;
    let mut args = args.into_iter();

    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--config" => {
                config_path = PathBuf::from(
                    args.next()
                        .ok_or_else(|| String::from("missing value for --config"))?,
                );
            }
            "--format" => {
                let value = args
                    .next()
                    .ok_or_else(|| String::from("missing value for --format"))?;
                format = parse_autostart_format(&value)?;
            }
            "--output" => {
                output_path = Some(PathBuf::from(
                    args.next()
                        .ok_or_else(|| String::from("missing value for --output"))?,
                ));
            }
            "--help" | "-h" => return Ok((config_path, format, output_path)),
            other => return Err(format!("unknown argument: {other}")),
        }
    }

    Ok((config_path, format, output_path))
}

fn parse_dns_command_args(
    args: Vec<String>,
) -> Result<(PathBuf, DnsFormat, Option<PathBuf>), String> {
    let mut config_path = default_config_path();
    let mut format = DnsFormat::Resolver;
    let mut output_path = None;
    let mut args = args.into_iter();

    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--config" => {
                config_path = PathBuf::from(
                    args.next()
                        .ok_or_else(|| String::from("missing value for --config"))?,
                );
            }
            "--format" => {
                let value = args
                    .next()
                    .ok_or_else(|| String::from("missing value for --format"))?;
                format = parse_dns_format(&value)?;
            }
            "--output" => {
                output_path = Some(PathBuf::from(
                    args.next()
                        .ok_or_else(|| String::from("missing value for --output"))?,
                ));
            }
            "--help" | "-h" => return Ok((config_path, format, output_path)),
            other => return Err(format!("unknown argument: {other}")),
        }
    }

    Ok((config_path, format, output_path))
}

fn parse_dns_format(value: &str) -> Result<DnsFormat, String> {
    match value {
        "resolver" => Ok(DnsFormat::Resolver),
        "resolved" => Ok(DnsFormat::Resolved),
        _ => Err(format!("unsupported dns format: {value}")),
    }
}

fn parse_autostart_format(value: &str) -> Result<AutostartFormat, String> {
    match value {
        "launchd" => Ok(AutostartFormat::Launchd),
        "systemd-user" => Ok(AutostartFormat::SystemdUser),
        _ => Err(format!("unsupported autostart format: {value}")),
    }
}

fn parse_init(args: Vec<String>) -> Result<Command, String> {
    let mut config_path = default_config_path();
    let mut network = String::from("nime");
    let mut dns_server: SocketAddr = "127.0.0.1:5354"
        .parse()
        .map_err(|_| String::from("invalid default dns server"))?;
    let mut dns_zone = String::from("nime.internal");
    let mut innernet_network = String::from("nime");
    let mut innernet_binary = String::from("innernet");
    let mut wg_userspace_implementation = None;

    let mut args = args.into_iter();
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--config" => {
                config_path = PathBuf::from(
                    args.next()
                        .ok_or_else(|| String::from("missing value for --config"))?,
                );
            }
            "--network" => {
                network = args
                    .next()
                    .ok_or_else(|| String::from("missing value for --network"))?;
            }
            "--dns-server" => {
                let value = args
                    .next()
                    .ok_or_else(|| String::from("missing value for --dns-server"))?;
                dns_server = value
                    .parse()
                    .map_err(|_| format!("invalid dns server address: {value}"))?;
            }
            "--dns-zone" => {
                dns_zone = normalize_zone(
                    &args
                        .next()
                        .ok_or_else(|| String::from("missing value for --dns-zone"))?,
                );
            }
            "--innernet-network" => {
                innernet_network = args
                    .next()
                    .ok_or_else(|| String::from("missing value for --innernet-network"))?;
            }
            "--innernet-bin" => {
                innernet_binary = args
                    .next()
                    .ok_or_else(|| String::from("missing value for --innernet-bin"))?;
            }
            "--wg-userspace-bin" => {
                wg_userspace_implementation = Some(
                    args.next()
                        .ok_or_else(|| String::from("missing value for --wg-userspace-bin"))?,
                );
            }
            "--help" | "-h" => return Ok(Command::Help),
            other => return Err(format!("unknown argument: {other}")),
        }
    }

    if network.trim().is_empty() {
        return Err(String::from("network must not be empty"));
    }
    if dns_zone.is_empty() {
        return Err(String::from("dns zone must not be empty"));
    }
    if innernet_network.trim().is_empty() {
        return Err(String::from("innernet network must not be empty"));
    }
    if innernet_binary.trim().is_empty() {
        return Err(String::from("innernet binary must not be empty"));
    }

    Ok(Command::Init {
        config_path,
        network,
        dns_server,
        dns_zone,
        innernet_network,
        innernet_binary,
        wg_userspace_implementation,
    })
}

fn parse_join(args: Vec<String>) -> Result<Command, String> {
    let mut config_path = default_config_path();
    let mut invitation = None;
    let mut name = None;
    let mut args = args.into_iter();

    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--config" => {
                config_path = PathBuf::from(
                    args.next()
                        .ok_or_else(|| String::from("missing value for --config"))?,
                );
            }
            "--invite" => {
                invitation = Some(PathBuf::from(
                    args.next()
                        .ok_or_else(|| String::from("missing value for --invite"))?,
                ));
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

    Ok(Command::Join {
        config_path,
        invitation: invitation.ok_or_else(|| String::from("missing required --invite"))?,
        name,
    })
}

fn parse_peers(args: Vec<String>) -> Result<Command, String> {
    let mut config_path = default_config_path();
    let mut tree = false;
    let mut args = args.into_iter();

    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--config" => {
                config_path = PathBuf::from(
                    args.next()
                        .ok_or_else(|| String::from("missing value for --config"))?,
                );
            }
            "--tree" => tree = true,
            "--help" | "-h" => return Ok(Command::Help),
            other => return Err(format!("unknown argument: {other}")),
        }
    }

    Ok(Command::Peers { config_path, tree })
}

fn print_help() {
    println!("nime-client commands:");
    println!(
        "  init --network nime --innernet-network nime --dns-server 10.60.0.1:5354 --dns-zone nime.internal"
    );
    println!("  status [--config client/nime-client.conf]");
    println!("  show-config [--config client/nime-client.conf]");
    println!("  join --invite invitation.toml [--name nime]");
    println!("  peers [--tree]");
    println!("  fetch");
    println!("  up");
    println!("  disconnect [--dns-format resolver|resolved] [--dns-output path] [--keep-dns]");
    println!("  doctor");
    println!("  dns-print [--format resolver|resolved]");
    println!("  dns-install [--format resolver|resolved] [--output path]");
    println!(
        "  setup --invite invitation.toml [--name nime] [--dns-format resolver|resolved] [--dns-output path] [--skip-dns]"
    );
    println!("  reconnect [--dns-format resolver|resolved] [--dns-output path] [--skip-dns]");
    println!("  autostart-print [--format launchd|systemd-user]");
    println!("  autostart-install [--format launchd|systemd-user] [--output path]");
    println!("  resolve <name>");
    println!("  services [query] [--json]");
    println!("  health [service] [--port PORT]");
    println!("  health --peer peer-name [--port PORT]");
    println!("  open [service] [--port PORT] [--https]");
}

fn default_config_path() -> PathBuf {
    PathBuf::from("client/nime-client.conf")
}

fn normalize_zone(zone: &str) -> String {
    zone.trim().trim_end_matches('.').to_ascii_lowercase()
}

fn write_config(path: &Path, config: &ClientConfig) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .map_err(|error| format!("failed to create {}: {error}", parent.display()))?;
    }

    fs::write(path, config.to_file_contents())
        .map_err(|error| format!("failed to write {}: {error}", path.display()))
}

fn read_config(path: &Path) -> Result<ClientConfig, String> {
    let contents = fs::read_to_string(path)
        .map_err(|error| format!("failed to read {}: {error}", path.display()))?;
    parse_config(&contents)
}

fn parse_config(contents: &str) -> Result<ClientConfig, String> {
    let mut network = None;
    let mut dns_server = None;
    let mut dns_zone = None;
    let mut innernet_network = None;
    let mut innernet_binary = None;
    let mut wg_userspace_implementation = None;

    for (line_number, raw_line) in contents.lines().enumerate() {
        let line = raw_line.split('#').next().unwrap_or("").trim();
        if line.is_empty() {
            continue;
        }

        let Some((key, value)) = line.split_once('=') else {
            return Err(format!("invalid config line {}", line_number + 1));
        };
        let key = key.trim();
        let value = value.trim();

        match key {
            "network" => network = Some(value.to_owned()),
            "innernet_network" => innernet_network = Some(value.to_owned()),
            "innernet_binary" => innernet_binary = Some(value.to_owned()),
            "wg_userspace_implementation" => {
                if !value.is_empty() {
                    wg_userspace_implementation = Some(value.to_owned());
                }
            }
            "dns_server" => {
                dns_server = Some(
                    value
                        .parse()
                        .map_err(|_| format!("invalid dns_server on line {}", line_number + 1))?,
                )
            }
            "dns_zone" => dns_zone = Some(normalize_zone(value)),
            other => {
                return Err(format!(
                    "unknown config key `{other}` on line {}",
                    line_number + 1
                ));
            }
        }
    }

    Ok(ClientConfig {
        network: required_field(network, "network")?,
        dns_server: required_field(dns_server, "dns_server")?,
        dns_zone: required_field(dns_zone, "dns_zone")?,
        innernet_network: required_field(innernet_network, "innernet_network")?,
        innernet_binary: innernet_binary.unwrap_or_else(|| String::from("innernet")),
        wg_userspace_implementation,
    })
}

fn required_field<T>(value: Option<T>, field: &str) -> Result<T, String> {
    value.ok_or_else(|| format!("missing required config field `{field}`"))
}

fn print_status(path: &Path, config: &ClientConfig) {
    let dns_reachable = check_udp_reachability(config.dns_server);
    let innernet_available = check_innernet_binary(&config.innernet_binary);
    let wg_userspace = resolved_wg_userspace_implementation(config);

    println!("config: {}", path.display());
    println!("network: {}", config.network);
    println!("innernet_network: {}", config.innernet_network);
    println!("innernet_binary: {}", config.innernet_binary);
    println!(
        "wg_userspace_implementation: {}",
        wg_userspace.as_deref().unwrap_or("(auto: not found)")
    );
    println!("dns_zone: {}", config.dns_zone);
    println!("dns_server: {}", config.dns_server);
    println!(
        "dns_reachable: {}",
        if dns_reachable { "yes" } else { "no" }
    );
    println!(
        "innernet_available: {}",
        if innernet_available { "yes" } else { "no" }
    );
}

fn run_doctor(path: &Path, config: &ClientConfig) {
    let dns_reachable = check_udp_reachability(config.dns_server);
    let innernet_available = check_innernet_binary(&config.innernet_binary);
    let wg_userspace = resolved_wg_userspace_implementation(config);
    let invitation_cmd = format!(
        "{} install <invite> --name {}",
        config.innernet_binary, config.innernet_network
    );

    println!("doctor report");
    println!("config: {}", path.display());
    println!(
        "dns_socket: {} ({})",
        config.dns_server,
        if dns_reachable {
            "reachable"
        } else {
            "unreachable"
        }
    );
    println!(
        "innernet_binary: {} ({})",
        config.innernet_binary,
        if innernet_available {
            "available"
        } else {
            "missing"
        }
    );
    println!(
        "wg_userspace_implementation: {}",
        wg_userspace.as_deref().unwrap_or("(auto: not found)")
    );
    println!(
        "dns_zone: {} (resolver target file: {})",
        config.dns_zone,
        resolver_target_path(&config.dns_zone)
    );
    println!("next_join_command: cargo run -p nime-client -- join --invite invitation.toml");
    println!("innernet_install_equivalent: {invitation_cmd}");
}

fn print_config(path: &Path, config: &ClientConfig) {
    println!("config: {}", path.display());
    println!("{}", config.to_file_contents().trim_end());
}

fn print_dns_config(config: &ClientConfig, format: DnsFormat) {
    match format {
        DnsFormat::Resolver => {
            println!("# macOS /etc/resolver/{}", config.dns_zone);
            println!("{}", render_resolver_file(config));
        }
        DnsFormat::Resolved => {
            println!(
                "# systemd-resolved drop-in for domain {}\n{}",
                config.dns_zone,
                render_systemd_resolved_snippet(config)
            );
        }
    }
}

fn install_dns_config(
    config: &ClientConfig,
    format: DnsFormat,
    output_path: Option<&Path>,
) -> Result<(), String> {
    let (path, contents) = match format {
        DnsFormat::Resolver => (
            output_path
                .map(PathBuf::from)
                .unwrap_or_else(|| PathBuf::from(resolver_target_path(&config.dns_zone))),
            render_resolver_file(config),
        ),
        DnsFormat::Resolved => (
            output_path
                .map(PathBuf::from)
                .unwrap_or_else(|| default_resolved_output_path(config)),
            render_systemd_resolved_snippet(config),
        ),
    };

    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .map_err(|error| format!("failed to create {}: {error}", parent.display()))?;
    }
    fs::write(&path, contents)
        .map_err(|error| format!("failed to write DNS config {}: {error}", path.display()))?;
    println!("installed DNS config at {}", path.display());
    Ok(())
}

fn remove_dns_config(
    config: &ClientConfig,
    format: DnsFormat,
    output_path: Option<&Path>,
) -> Result<(), String> {
    let path = match format {
        DnsFormat::Resolver => output_path
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from(resolver_target_path(&config.dns_zone))),
        DnsFormat::Resolved => output_path
            .map(PathBuf::from)
            .unwrap_or_else(|| default_resolved_output_path(config)),
    };

    if path.exists() {
        fs::remove_file(&path)
            .map_err(|error| format!("failed to remove DNS config {}: {error}", path.display()))?;
        println!("removed DNS config at {}", path.display());
    } else {
        println!("DNS config not found at {}, skipping", path.display());
    }

    Ok(())
}

fn registry_api_addr(config: &ClientConfig) -> SocketAddr {
    SocketAddr::new(config.dns_server.ip(), 4172)
}

#[derive(Debug, Clone)]
struct ServiceSummary {
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

fn print_autostart_config(config: &ClientConfig, config_path: &Path, format: AutostartFormat) {
    match format {
        AutostartFormat::Launchd => println!("{}", render_launchd_plist(config, config_path)),
        AutostartFormat::SystemdUser => {
            println!("{}", render_systemd_user_unit(config, config_path))
        }
    }
}

fn install_autostart_config(
    config: &ClientConfig,
    config_path: &Path,
    format: AutostartFormat,
    output_path: Option<&Path>,
) -> Result<(), String> {
    let (path, contents) = match format {
        AutostartFormat::Launchd => (
            output_path
                .map(PathBuf::from)
                .unwrap_or_else(|| default_launchd_output_path(config)),
            render_launchd_plist(config, config_path),
        ),
        AutostartFormat::SystemdUser => (
            output_path
                .map(PathBuf::from)
                .unwrap_or_else(|| default_systemd_user_output_path(config)),
            render_systemd_user_unit(config, config_path),
        ),
    };

    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .map_err(|error| format!("failed to create {}: {error}", parent.display()))?;
    }
    fs::write(&path, contents).map_err(|error| {
        format!(
            "failed to write autostart config {}: {error}",
            path.display()
        )
    })?;
    println!("installed autostart config at {}", path.display());
    Ok(())
}

fn check_udp_reachability(addr: SocketAddr) -> bool {
    let bind_addr = if addr.is_ipv4() {
        "0.0.0.0:0"
    } else {
        "[::]:0"
    };
    let socket = match UdpSocket::bind(bind_addr) {
        Ok(socket) => socket,
        Err(_) => return false,
    };

    if socket
        .set_write_timeout(Some(Duration::from_millis(500)))
        .is_err()
    {
        return false;
    }

    socket.connect(addr).is_ok()
        && socket.send(&build_probe_query()).is_ok()
        && socket
            .set_read_timeout(Some(Duration::from_millis(500)))
            .is_ok()
        && {
            let mut buffer = [0_u8; 512];
            socket.recv(&mut buffer).is_ok()
        }
}

fn resolve_a_record(server: SocketAddr, name: &str) -> Result<Vec<Ipv4Addr>, String> {
    let bind_addr = if server.is_ipv4() {
        "0.0.0.0:0"
    } else {
        "[::]:0"
    };
    let socket = UdpSocket::bind(bind_addr)
        .map_err(|error| format!("failed to bind UDP socket: {error}"))?;
    socket
        .set_read_timeout(Some(Duration::from_secs(1)))
        .map_err(|error| format!("failed to set read timeout: {error}"))?;
    socket
        .set_write_timeout(Some(Duration::from_secs(1)))
        .map_err(|error| format!("failed to set write timeout: {error}"))?;
    socket
        .connect(server)
        .map_err(|error| format!("failed to connect to DNS server {server}: {error}"))?;

    let request = build_a_query(name)?;
    socket
        .send(&request)
        .map_err(|error| format!("failed to send DNS request to {server}: {error}"))?;

    let mut buffer = [0_u8; 512];
    let size = socket
        .recv(&mut buffer)
        .map_err(|error| format!("failed to read DNS response from {server}: {error}"))?;
    parse_a_response(&buffer[..size])
}

fn build_a_query(name: &str) -> Result<Vec<u8>, String> {
    let mut bytes = vec![
        0x13, 0x37, 0x01, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    ];
    write_dns_name(&mut bytes, name)?;
    bytes.extend_from_slice(&[0x00, 0x01, 0x00, 0x01]);
    Ok(bytes)
}

fn write_dns_name(bytes: &mut Vec<u8>, name: &str) -> Result<(), String> {
    for label in name.trim_end_matches('.').split('.') {
        if label.is_empty() {
            return Err(format!("invalid DNS name: {name}"));
        }
        if label.len() > 63 {
            return Err(format!("DNS label too long in {name}"));
        }
        bytes.push(label.len() as u8);
        bytes.extend_from_slice(label.as_bytes());
    }
    bytes.push(0);
    Ok(())
}

fn parse_a_response(response: &[u8]) -> Result<Vec<Ipv4Addr>, String> {
    if response.len() < 12 {
        return Err(String::from("short DNS response"));
    }
    let flags = u16::from_be_bytes([response[2], response[3]]);
    if flags & 0x000f != 0 {
        return Err(format!("DNS server returned rcode {}", flags & 0x000f));
    }

    let questions = u16::from_be_bytes([response[4], response[5]]) as usize;
    let answers = u16::from_be_bytes([response[6], response[7]]) as usize;
    let mut offset = 12;

    for _ in 0..questions {
        offset = skip_dns_name(response, offset)?;
        offset = offset
            .checked_add(4)
            .filter(|offset| *offset <= response.len())
            .ok_or_else(|| String::from("truncated DNS question"))?;
    }

    let mut records = Vec::new();
    for _ in 0..answers {
        offset = skip_dns_name(response, offset)?;
        if offset + 10 > response.len() {
            return Err(String::from("truncated DNS answer header"));
        }
        let record_type = u16::from_be_bytes([response[offset], response[offset + 1]]);
        let record_class = u16::from_be_bytes([response[offset + 2], response[offset + 3]]);
        let rdlength = u16::from_be_bytes([response[offset + 8], response[offset + 9]]) as usize;
        offset += 10;
        if offset + rdlength > response.len() {
            return Err(String::from("truncated DNS answer body"));
        }
        if record_type == 1 && record_class == 1 && rdlength == 4 {
            records.push(Ipv4Addr::new(
                response[offset],
                response[offset + 1],
                response[offset + 2],
                response[offset + 3],
            ));
        }
        offset += rdlength;
    }

    Ok(records)
}

fn skip_dns_name(response: &[u8], mut offset: usize) -> Result<usize, String> {
    loop {
        let Some(length) = response.get(offset).copied() else {
            return Err(String::from("truncated DNS name"));
        };
        if length & 0xc0 == 0xc0 {
            return offset
                .checked_add(2)
                .filter(|offset| *offset <= response.len())
                .ok_or_else(|| String::from("truncated DNS pointer"));
        }
        offset += 1;
        if length == 0 {
            return Ok(offset);
        }
        offset = offset
            .checked_add(length as usize)
            .filter(|offset| *offset <= response.len())
            .ok_or_else(|| String::from("truncated DNS label"))?;
    }
}

fn check_innernet_binary(innernet_binary: &str) -> bool {
    ProcessCommand::new(innernet_binary)
        .arg("--help")
        .output()
        .map(|output| output.status.success())
        .unwrap_or(false)
}

fn resolved_wg_userspace_implementation(config: &ClientConfig) -> Option<String> {
    config
        .wg_userspace_implementation
        .clone()
        .or_else(|| find_binary_in_path("wireguard-go"))
}

fn find_binary_in_path(name: &str) -> Option<String> {
    let paths = env::var_os("PATH")?;
    for dir in env::split_paths(&paths) {
        let candidate = dir.join(name);
        if candidate.is_file() {
            return Some(candidate.display().to_string());
        }
    }
    None
}

fn resolver_target_path(zone: &str) -> String {
    format!("/etc/resolver/{zone}")
}

fn default_resolved_output_path(config: &ClientConfig) -> PathBuf {
    PathBuf::from(format!(
        "/etc/systemd/resolved.conf.d/{}.conf",
        config.network
    ))
}

fn default_launchd_output_path(config: &ClientConfig) -> PathBuf {
    PathBuf::from(format!(
        "/Library/LaunchDaemons/com.nime.client.{}.plist",
        config.network
    ))
}

fn default_systemd_user_output_path(config: &ClientConfig) -> PathBuf {
    PathBuf::from(format!(
        "{}/.config/systemd/user/nime-client-{}.service",
        std::env::var("HOME").unwrap_or_else(|_| String::from(".")),
        config.network
    ))
}

fn render_launchd_plist(config: &ClientConfig, config_path: &Path) -> String {
    let binary = current_binary_path();
    let env_block = resolved_wg_userspace_implementation(config)
        .as_ref()
        .map(|path| {
            format!(
                "<key>EnvironmentVariables</key><dict><key>WG_USERSPACE_IMPLEMENTATION</key><string>{}</string></dict>",
                escape_xml(path)
            )
        })
        .unwrap_or_default();

    format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
  <key>Label</key>
  <string>com.nime.client.{network}</string>
  <key>ProgramArguments</key>
  <array>
    <string>{binary}</string>
    <string>reconnect</string>
    <string>--config</string>
    <string>{config}</string>
  </array>
  {env_block}
  <key>RunAtLoad</key>
  <true/>
  <key>KeepAlive</key>
  <true/>
  <key>StandardOutPath</key>
  <string>/tmp/nime-client-{network}.out.log</string>
  <key>StandardErrorPath</key>
  <string>/tmp/nime-client-{network}.err.log</string>
</dict>
</plist>"#,
        network = escape_xml(&config.network),
        binary = escape_xml(&binary),
        config = escape_xml(&config_path.display().to_string()),
        env_block = env_block,
    )
}

fn render_systemd_user_unit(config: &ClientConfig, config_path: &Path) -> String {
    let binary = current_binary_path();
    let env_line = resolved_wg_userspace_implementation(config)
        .as_ref()
        .map(|path| format!("Environment=WG_USERSPACE_IMPLEMENTATION={path}\n"))
        .unwrap_or_default();

    format!(
        "[Unit]\nDescription=Nime client reconnect for {network}\nAfter=network-online.target\nWants=network-online.target\n\n[Service]\nType=oneshot\nExecStart={binary} reconnect --config {config}\n{env}[Install]\nWantedBy=default.target\n",
        network = config.network,
        binary = binary,
        config = config_path.display(),
        env = env_line
    )
}

fn current_binary_path() -> String {
    std::env::current_exe()
        .ok()
        .map(|path| path.display().to_string())
        .unwrap_or_else(|| String::from("nime-client"))
}

fn escape_xml(input: &str) -> String {
    input
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

fn render_resolver_file(config: &ClientConfig) -> String {
    format!(
        "nameserver {}\nport {}\ndomain {}\nsearch_order 1\n",
        config.dns_server.ip(),
        config.dns_server.port(),
        config.dns_zone
    )
}

fn render_systemd_resolved_snippet(config: &ClientConfig) -> String {
    format!(
        "[Resolve]\nDNS={}#{}\nDomains=~{}\n",
        config.dns_server.ip(),
        config.dns_server.port(),
        config.dns_zone
    )
}

fn print_services(config: &ClientConfig, query: Option<&str>, json: bool) -> Result<(), String> {
    let api_addr = registry_api_addr(config);
    let services = fetch_services(config, query)?;

    if services.is_empty() {
        println!("services: none");
        return Ok(());
    }

    if json {
        println!(
            "{}",
            serde_json::to_string_pretty(&http_get_json(api_addr, &services_target(query))?)
                .map_err(|error| format!("failed to render JSON output: {error}"))?
        );
        return Ok(());
    }

    for service in services {
        let fqdn = service_fqdn(&service, &config.dns_zone);
        let url = service_url(&service, &fqdn);
        let tags = if service.tags.is_empty() {
            String::from("-")
        } else {
            service.tags.join(",")
        };
        let description = if service.description.trim().is_empty() {
            String::from("-")
        } else {
            service.description.clone()
        };

        println!("{} [{}]", service.title, fqdn);
        println!("url: {url}");
        println!("group: {}", service.group_name);
        println!(
            "target: {}://{}:{}",
            service.protocol, service.target_ip, service.port
        );
        println!("owner: {}", service.owner);
        println!("tags: {tags}");
        println!("description: {description}");
        println!();
    }

    Ok(())
}

fn absolute_name(name: &str, zone: &str) -> String {
    let trimmed = normalize_zone(name);
    if trimmed.ends_with(zone) {
        trimmed
    } else {
        format!("{trimmed}.{zone}")
    }
}

fn run_health(
    config: &ClientConfig,
    service: Option<&str>,
    peer: Option<&str>,
    port: Option<u16>,
) -> Result<(), String> {
    if let Some(peer) = peer {
        return run_peer_health(config, peer, port);
    }

    let service = service.unwrap_or("portal");
    let api_addr = registry_api_addr(config);
    let service_meta = fetch_service(config, service)?;
    let fqdn = service_meta
        .as_ref()
        .map(|service| service_fqdn(service, &config.dns_zone))
        .unwrap_or_else(|| absolute_name(service, &config.dns_zone));
    let records = resolve_a_record(config.dns_server, &fqdn)?;
    let Some(ip) = records.first() else {
        return Err(format!("no A records for {fqdn}"));
    };
    let port = port.unwrap_or_else(|| {
        service_meta
            .as_ref()
            .map(|service| service.port)
            .unwrap_or(8080)
    });
    let addr = SocketAddr::from((*ip, port));

    println!("registry_api: {api_addr} (reachable)");
    if let Some(service) = &service_meta {
        println!("service: {} [{}]", service.title, service.name);
        println!("service_url: {}", service_url(service, &fqdn));
    }
    println!("dns: {fqdn} -> {ip}");
    println!("tcp: connecting to {addr}");

    let mut stream = TcpStream::connect_timeout(&addr, Duration::from_secs(2))
        .map_err(|error| format!("failed to connect to {addr}: {error}"))?;
    stream
        .set_read_timeout(Some(Duration::from_secs(2)))
        .map_err(|error| format!("failed to set read timeout: {error}"))?;
    stream
        .set_write_timeout(Some(Duration::from_secs(2)))
        .map_err(|error| format!("failed to set write timeout: {error}"))?;

    let request = format!("GET /health HTTP/1.1\r\nHost: {fqdn}\r\nConnection: close\r\n\r\n");
    stream
        .write_all(request.as_bytes())
        .map_err(|error| format!("failed to write HTTP request: {error}"))?;

    let mut response = String::new();
    stream
        .read_to_string(&mut response)
        .map_err(|error| format!("failed to read HTTP response: {error}"))?;

    let status_line = response.lines().next().unwrap_or("");
    println!("http: {status_line}");
    if !status_line.contains("200") {
        return Err(format!(
            "health endpoint returned unexpected status: {status_line}"
        ));
    }
    println!("health: ok");
    Ok(())
}

fn run_peer_health(config: &ClientConfig, peer: &str, port: Option<u16>) -> Result<(), String> {
    let fqdn = absolute_name(peer, &config.dns_zone);
    let records = resolve_a_record(config.dns_server, &fqdn)?;
    let Some(ip) = records.first() else {
        return Err(format!("no A records for {fqdn}"));
    };
    let port = port.unwrap_or(22);
    let addr = SocketAddr::from((*ip, port));

    println!("peer: {peer}");
    println!("dns: {fqdn} -> {ip}");
    println!("tcp: connecting to {addr}");

    TcpStream::connect_timeout(&addr, Duration::from_secs(2))
        .map_err(|error| format!("failed to connect to peer {addr}: {error}"))?;

    println!("peer_health: ok");
    Ok(())
}

fn fetch_services(
    config: &ClientConfig,
    query: Option<&str>,
) -> Result<Vec<ServiceSummary>, String> {
    let api_addr = registry_api_addr(config);
    let body = http_get_json(api_addr, &services_target(query))?;
    let services = body
        .as_array()
        .ok_or_else(|| String::from("registry API returned unexpected payload"))?;
    services
        .iter()
        .map(service_summary_from_json)
        .collect::<Result<Vec<_>, _>>()
}

fn fetch_service(config: &ClientConfig, name: &str) -> Result<Option<ServiceSummary>, String> {
    let identifier = normalize_zone(name);
    if identifier.contains('.') {
        return fetch_services(config, None).map(|services| {
            services
                .into_iter()
                .find(|service| service_matches_identifier(service, &identifier, &config.dns_zone))
        });
    }

    let api_addr = registry_api_addr(config);
    let target = format!("/services/{}", identifier);
    match http_get_json(api_addr, &target) {
        Ok(body) => service_summary_from_json(&body).map(Some),
        Err(error) if error.contains("404") => Ok(None),
        Err(error) => Err(error),
    }
}

fn services_target(query: Option<&str>) -> String {
    let mut target = String::from("/services");
    if let Some(query) = query.filter(|value| !value.trim().is_empty()) {
        target.push_str("?q=");
        target.push_str(&url_encode(query));
    }
    target
}

fn service_summary_from_json(service: &Value) -> Result<ServiceSummary, String> {
    Ok(ServiceSummary {
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

fn service_matches_identifier(service: &ServiceSummary, identifier: &str, zone: &str) -> bool {
    identifier == service.name
        || identifier == format!("{}.{}", service.name, service.group_name)
        || identifier == service_fqdn(service, zone)
}

fn service_fqdn(service: &ServiceSummary, zone: &str) -> String {
    format!("{}.{}.{}", service.name, service.group_name, zone)
}

fn service_url(service: &ServiceSummary, fqdn: &str) -> String {
    let default_port = (service.protocol == "http" && service.port == 80)
        || (service.protocol == "https" && service.port == 443);
    if default_port {
        format!("{}://{fqdn}", service.protocol)
    } else {
        format!("{}://{fqdn}:{}", service.protocol, service.port)
    }
}

fn open_service(
    config: &ClientConfig,
    service: &str,
    port: Option<u16>,
    https: bool,
) -> Result<(), String> {
    let service_meta = fetch_service(config, service)?;
    let fqdn = service_meta
        .as_ref()
        .map(|service| service_fqdn(service, &config.dns_zone))
        .unwrap_or_else(|| absolute_name(service, &config.dns_zone));
    let scheme = if https {
        "https"
    } else {
        service_meta
            .as_ref()
            .map(|service| service.protocol.as_str())
            .unwrap_or("http")
    };
    let mut url = format!("{scheme}://{fqdn}");
    let resolved_port = port.or_else(|| service_meta.as_ref().map(|service| service.port));
    if let Some(port) = resolved_port {
        url.push(':');
        url.push_str(&port.to_string());
    }

    let mut command = if cfg!(target_os = "macos") {
        let mut command = ProcessCommand::new("open");
        command.arg(&url);
        command
    } else if cfg!(target_os = "linux") {
        let mut command = ProcessCommand::new("xdg-open");
        command.arg(&url);
        command
    } else {
        return Err(format!(
            "unsupported OS for automatic open; use this URL manually: {url}"
        ));
    };

    let status = command
        .status()
        .map_err(|error| format!("failed to open browser for {url}: {error}"))?;
    if !status.success() {
        return Err(format!(
            "browser open command failed for {url} with status {status}"
        ));
    }

    println!("opened {url}");
    Ok(())
}

fn http_get_json(addr: SocketAddr, target: &str) -> Result<Value, String> {
    let mut stream = TcpStream::connect_timeout(&addr, Duration::from_secs(2))
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

fn run_innernet_install(
    config: &ClientConfig,
    invitation: &Path,
    name: Option<&str>,
) -> Result<(), String> {
    let mut args = vec![String::from("install"), invitation.display().to_string()];
    let interface_name = name.unwrap_or(&config.innernet_network);
    args.push(String::from("--name"));
    args.push(interface_name.to_owned());

    run_innernet(config, &args)
}

fn run_innernet_list(config: &ClientConfig, tree: bool) -> Result<(), String> {
    let mut args = vec![String::from("list"), config.innernet_network.clone()];
    if tree {
        args.push(String::from("--tree"));
    }
    run_innernet(config, &args)
}

fn run_innernet_interface_command(
    config: &ClientConfig,
    subcommand: &str,
    extra_args: &[String],
) -> Result<(), String> {
    let mut args = vec![String::from(subcommand), config.innernet_network.clone()];
    args.extend(extra_args.iter().cloned());
    run_innernet(config, &args)
}

fn run_innernet(config: &ClientConfig, args: &[String]) -> Result<(), String> {
    let mut command = ProcessCommand::new(&config.innernet_binary);
    if let Some(path) = resolved_wg_userspace_implementation(config) {
        command.env("WG_USERSPACE_IMPLEMENTATION", path);
    }

    let status = command
        .args(args)
        .status()
        .map_err(|error| format!("failed to execute `{}`: {error}", config.innernet_binary))?;

    if status.success() {
        Ok(())
    } else {
        Err(format!(
            "`{} {}` failed with status {status}",
            config.innernet_binary,
            args.join(" ")
        ))
    }
}

fn build_probe_query() -> [u8; 31] {
    [
        0x13, 0x37, 0x01, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x04, b'p', b'i',
        b'n', b'g', 0x08, b'i', b'n', b't', b'e', b'r', b'n', b'a', b'l', 0x00, 0x00, 0x01, 0x00,
        0x01,
    ]
}

impl fmt::Display for ClientConfig {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "network={} innernet_network={} innernet_binary={} dns_server={} dns_zone={}",
            self.network,
            self.innernet_network,
            self.innernet_binary,
            self.dns_server,
            self.dns_zone
        )?;
        if let Some(path) = &self.wg_userspace_implementation {
            write!(f, " wg_userspace_implementation={path}")?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_config_file() {
        let config = parse_config(
            "network = nime\ninnernet_network = nime\ninnernet_binary = innernet\ndns_server = 10.60.0.1:5354\ndns_zone = nime.internal\n",
        )
        .unwrap();

        assert_eq!(config.network, "nime");
        assert_eq!(config.innernet_network, "nime");
        assert_eq!(config.innernet_binary, "innernet");
        assert_eq!(config.dns_server, "10.60.0.1:5354".parse().unwrap());
        assert_eq!(config.dns_zone, "nime.internal");
        assert_eq!(config.wg_userspace_implementation, None);
    }

    #[test]
    fn init_defaults_are_valid() {
        let command = parse_cli(vec![String::from("init")]).unwrap();
        match command {
            Command::Init {
                network,
                dns_server,
                dns_zone,
                innernet_network,
                innernet_binary,
                ..
            } => {
                assert_eq!(network, "nime");
                assert_eq!(dns_server, "127.0.0.1:5354".parse().unwrap());
                assert_eq!(dns_zone, "nime.internal");
                assert_eq!(innernet_network, "nime");
                assert_eq!(innernet_binary, "innernet");
            }
            _ => panic!("expected init command"),
        }
    }

    #[test]
    fn parses_join_command() {
        let command = parse_cli(vec![
            String::from("join"),
            String::from("--invite"),
            String::from("invite.toml"),
            String::from("--name"),
            String::from("nime"),
        ])
        .unwrap();

        match command {
            Command::Join {
                invitation, name, ..
            } => {
                assert_eq!(invitation, PathBuf::from("invite.toml"));
                assert_eq!(name.as_deref(), Some("nime"));
            }
            _ => panic!("expected join command"),
        }
    }

    #[test]
    fn parses_dns_print_command() {
        let command = parse_cli(vec![
            String::from("dns-print"),
            String::from("--format"),
            String::from("resolved"),
        ])
        .unwrap();

        match command {
            Command::DnsPrint { format, .. } => assert_eq!(format, DnsFormat::Resolved),
            _ => panic!("expected dns-print command"),
        }
    }

    #[test]
    fn renders_resolver_file() {
        let config = ClientConfig {
            network: String::from("nime"),
            innernet_network: String::from("nime"),
            innernet_binary: String::from("innernet"),
            wg_userspace_implementation: None,
            dns_server: "10.60.0.1:5354".parse().unwrap(),
            dns_zone: String::from("nime.internal"),
        };

        let rendered = render_resolver_file(&config);
        assert!(rendered.contains("nameserver 10.60.0.1"));
        assert!(rendered.contains("port 5354"));
        assert!(rendered.contains("domain nime.internal"));
    }

    #[test]
    fn parses_setup_command() {
        let command = parse_cli(vec![
            String::from("setup"),
            String::from("--invite"),
            String::from("invite.toml"),
            String::from("--dns-format"),
            String::from("resolved"),
            String::from("--skip-dns"),
        ])
        .unwrap();

        match command {
            Command::Setup {
                invitation,
                dns_format,
                skip_dns,
                ..
            } => {
                assert_eq!(invitation, PathBuf::from("invite.toml"));
                assert_eq!(dns_format, DnsFormat::Resolved);
                assert!(skip_dns);
            }
            _ => panic!("expected setup command"),
        }
    }

    #[test]
    fn parses_init_with_wg_userspace_bin() {
        let command = parse_cli(vec![
            String::from("init"),
            String::from("--wg-userspace-bin"),
            String::from("/opt/homebrew/bin/wireguard-go"),
        ])
        .unwrap();

        match command {
            Command::Init {
                wg_userspace_implementation,
                ..
            } => {
                assert_eq!(
                    wg_userspace_implementation.as_deref(),
                    Some("/opt/homebrew/bin/wireguard-go")
                );
            }
            _ => panic!("expected init command"),
        }
    }

    #[test]
    fn parses_reconnect_command() {
        let command = parse_cli(vec![
            String::from("reconnect"),
            String::from("--dns-format"),
            String::from("resolved"),
            String::from("--skip-dns"),
        ])
        .unwrap();

        match command {
            Command::Reconnect {
                dns_format,
                skip_dns,
                ..
            } => {
                assert_eq!(dns_format, DnsFormat::Resolved);
                assert!(skip_dns);
            }
            _ => panic!("expected reconnect command"),
        }
    }

    #[test]
    fn parses_disconnect_command() {
        let command = parse_cli(vec![
            String::from("disconnect"),
            String::from("--dns-format"),
            String::from("resolved"),
            String::from("--keep-dns"),
        ])
        .unwrap();

        match command {
            Command::Disconnect {
                dns_format,
                keep_dns,
                ..
            } => {
                assert_eq!(dns_format, DnsFormat::Resolved);
                assert!(keep_dns);
            }
            _ => panic!("expected disconnect command"),
        }
    }

    #[test]
    fn parses_autostart_install_command() {
        let command = parse_cli(vec![
            String::from("autostart-install"),
            String::from("--format"),
            String::from("systemd-user"),
            String::from("--output"),
            String::from("/tmp/nime-client.service"),
        ])
        .unwrap();

        match command {
            Command::AutostartInstall {
                format,
                output_path,
                ..
            } => {
                assert_eq!(format, AutostartFormat::SystemdUser);
                assert_eq!(output_path, Some(PathBuf::from("/tmp/nime-client.service")));
            }
            _ => panic!("expected autostart-install command"),
        }
    }

    #[test]
    fn parses_resolve_command() {
        let command = parse_cli(vec![String::from("resolve"), String::from("portal")]).unwrap();

        match command {
            Command::Resolve { name, .. } => assert_eq!(name, "portal"),
            _ => panic!("expected resolve command"),
        }
    }

    #[test]
    fn parses_services_command() {
        let command = parse_cli(vec![String::from("services"), String::from("portal")]).unwrap();

        match command {
            Command::Services { query, json, .. } => {
                assert_eq!(query.as_deref(), Some("portal"));
                assert!(!json);
            }
            _ => panic!("expected services command"),
        }
    }

    #[test]
    fn parses_services_json_command() {
        let command = parse_cli(vec![String::from("services"), String::from("--json")]).unwrap();

        match command {
            Command::Services { query, json, .. } => {
                assert_eq!(query, None);
                assert!(json);
            }
            _ => panic!("expected services command"),
        }
    }

    #[test]
    fn parses_peer_health_command() {
        let command = parse_cli(vec![
            String::from("health"),
            String::from("--peer"),
            String::from("laptop"),
            String::from("--port"),
            String::from("22"),
        ])
        .unwrap();

        match command {
            Command::Health {
                service,
                peer,
                port,
                ..
            } => {
                assert_eq!(service, None);
                assert_eq!(peer.as_deref(), Some("laptop"));
                assert_eq!(port, Some(22));
            }
            _ => panic!("expected health command"),
        }
    }

    #[test]
    fn parses_open_command() {
        let command = parse_cli(vec![
            String::from("open"),
            String::from("portal"),
            String::from("--port"),
            String::from("8080"),
            String::from("--https"),
        ])
        .unwrap();

        match command {
            Command::Open {
                service,
                port,
                https,
                ..
            } => {
                assert_eq!(service.as_deref(), Some("portal"));
                assert_eq!(port, Some(8080));
                assert!(https);
            }
            _ => panic!("expected open command"),
        }
    }

    #[test]
    fn renders_launchd_plist_with_env() {
        let config = ClientConfig {
            network: String::from("nime"),
            innernet_network: String::from("nimenet"),
            innernet_binary: String::from("innernet"),
            wg_userspace_implementation: Some(String::from("/opt/homebrew/bin/wireguard-go")),
            dns_server: "10.42.0.1:5354".parse().unwrap(),
            dns_zone: String::from("nime.internal"),
        };

        let rendered = render_launchd_plist(&config, Path::new("client/nime-client.conf"));
        assert!(rendered.contains("WG_USERSPACE_IMPLEMENTATION"));
        assert!(rendered.contains("/opt/homebrew/bin/wireguard-go"));
        assert!(rendered.contains("reconnect"));
    }

    #[test]
    fn prefers_explicit_wg_userspace_path() {
        let config = ClientConfig {
            network: String::from("nime"),
            innernet_network: String::from("nimenet"),
            innernet_binary: String::from("innernet"),
            wg_userspace_implementation: Some(String::from("/custom/wireguard-go")),
            dns_server: "10.42.0.1:5354".parse().unwrap(),
            dns_zone: String::from("nime.internal"),
        };

        assert_eq!(
            resolved_wg_userspace_implementation(&config).as_deref(),
            Some("/custom/wireguard-go")
        );
    }

    #[test]
    fn absolute_name_appends_zone() {
        assert_eq!(
            absolute_name("portal", "nime.internal"),
            "portal.nime.internal"
        );
        assert_eq!(
            absolute_name("portal.nime.internal", "nime.internal"),
            "portal.nime.internal"
        );
    }

    #[test]
    fn parses_a_response_record() {
        let response = [
            0x13, 0x37, 0x81, 0x80, 0x00, 0x01, 0x00, 0x01, 0x00, 0x00, 0x00, 0x00, 0x06, b'p',
            b'o', b'r', b't', b'a', b'l', 0x04, b'n', b'i', b'm', b'e', 0x08, b'i', b'n', b't',
            b'e', b'r', b'n', b'a', b'l', 0x00, 0x00, 0x01, 0x00, 0x01, 0xc0, 0x0c, 0x00, 0x01,
            0x00, 0x01, 0x00, 0x00, 0x00, 0x3c, 0x00, 0x04, 10, 42, 0, 1,
        ];

        let records = parse_a_response(&response).unwrap();
        assert_eq!(records, vec![Ipv4Addr::new(10, 42, 0, 1)]);
    }

    #[test]
    fn builds_service_url() {
        let service = ServiceSummary {
            name: String::from("portal"),
            group_name: String::from("core"),
            target_ip: String::from("10.42.0.1"),
            port: 8080,
            protocol: String::from("http"),
            title: String::from("Portal"),
            description: String::new(),
            owner: String::from("arthur"),
            tags: vec![],
        };

        assert_eq!(
            service_url(&service, "portal.core.nime.internal"),
            "http://portal.core.nime.internal:8080"
        );
    }
}
