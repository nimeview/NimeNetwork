# Сборка И Артефакты

## Что нужно на Linux-сервере

Минимально:

- `cargo`
- `rustc`
- `sqlite3` dev headers
- `wireguard-tools`
- компилятор C/C++

Установка через helper:

```bash
cd ~/NimeNetwork
scripts/nime-server-linux.sh install-deps
```

Если Rust ещё не установлен:

```bash
curl https://sh.rustup.rs -sSf | sh
source "$HOME/.cargo/env"
```

## Сборка workspace

Из корня проекта:

```bash
cargo build
cargo test
```

Release:

```bash
cargo build --release
```

## Сборка vendored `innernet`

Серверный бинарник:

```bash
cd server/innernet
cargo build --release --bin innernet-server
```

Клиентский бинарник:

```bash
cd server/innernet
cargo build --release --bin innernet
```

## Сборка всего сервера одной командой

```bash
cd ~/NimeNetwork
scripts/nime-server-linux.sh build
```

Это собирает:

- `nime-registry`
- `nime-dns`
- `nime-portal`
- `nime-client`
- `innernet`
- `innernet-server`

## Release-архивы

Проект умеет собирать готовые tar.gz-пакеты:

```bash
scripts/package-release.sh
```

Результат:

- `dist/nime-client-<version>-<host>.tar.gz`
- `dist/nime-server-<version>-<host>.tar.gz`

Это удобно, если не хочется таскать весь репозиторий на целевую машину.

## mdBook

Документация теперь оформлена как `mdBook`.

Если `mdbook` установлен:

```bash
mdbook build
mdbook serve -o
```

Конфигурация лежит в:

- [book.toml](../../book.toml)

Исходники документации:

- `book/src/`
