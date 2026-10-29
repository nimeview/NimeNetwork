# NimeNetwork

Private network platform built around `innernet` for connecting machines, resolving
internal names, and publishing services inside a controlled mesh network.

This repository brings together the network layer, server-side utilities, a small
client CLI, and the service registry used by the platform.

## What is included

- `server/innernet` — vendored Rust source for the WireGuard-based mesh layer
- `server/dns` — internal DNS for peers and services
- `server/registry` — service registry and HTTP API
- `server/portal` — internal landing page and service search
- `client` — CLI for setup, reconnect, DNS, and quick network checks
- `services/` — small registry-backed services used by the network
- `book/src/` and `docs/` — operational and user documentation

## Quick start

The complete setup depends on the target operating system and requires elevated
privileges for the network interface. Start with [docs/QUICKSTART.md](docs/QUICKSTART.md)
or use the more detailed [RUNBOOK.md](RUNBOOK.md).

To build the Rust workspace:

```bash
cargo build --workspace
```

To run the tests:

```bash
cargo test --workspace --all-targets
```

The embedded `innernet` workspace is built separately from
`server/innernet/` when a client or server binary is required.

## Documentation

- [Quick start](docs/QUICKSTART.md)
- [User guide](docs/USER_GUIDE.md)
- [Administrator guide](docs/ADMIN_GUIDE.md)
- [Publishing a service](docs/PUBLISH_SERVICE.md)
- [Release workflow](docs/RELEASES.md)
- [Operational runbook](RUNBOOK.md)
- [Architecture book](book/src/README.md)

## Status

NimeNetwork is a private infrastructure project and remains under active
development. The repository is used to document and evolve the network topology,
service discovery flow, deployment scripts, and client operations.
