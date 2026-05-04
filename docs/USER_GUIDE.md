# Гайд Для Пользователя

Этот документ для участника сети, который просто хочет подключиться, открыть сервисы и при необходимости отключиться.

## Что нужно пользователю

- invitation-файл от администратора
- установленный `innernet`
- собранный `nime-client`

## Первое подключение

Инициализация:

```bash
cargo run -p nime-client -- init \
  --network nime \
  --innernet-network nimenet \
  --dns-server 10.42.0.1:5354 \
  --dns-zone nime.internal \
  --innernet-bin /absolute/path/to/innernet
```

Подключение:

```bash
sudo ./target/release/nime-client setup --invite invitation.toml
```

## Что делать каждый день

Посмотреть сервисы:

```bash
cargo run -p nime-client -- services
```

Открыть портал:

```bash
cargo run -p nime-client -- open portal --port 8080
```

Проверить сервис:

```bash
cargo run -p nime-client -- health portal
```

Проверить peer:

```bash
cargo run -p nime-client -- health --peer laptop --port 22
```

## После перезагрузки

```bash
sudo ./target/release/nime-client reconnect
```

## Если надо отключиться

```bash
sudo ./target/release/nime-client disconnect
```

Если DNS надо оставить:

```bash
sudo ./target/release/nime-client disconnect --keep-dns
```
