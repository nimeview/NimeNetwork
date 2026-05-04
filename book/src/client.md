# Клиент

Этот раздел для участника сети.

## Что нужно клиенту

- invitation-файл от администратора
- собранный `innernet`
- `nime-client`
- `wireguard-go` на macOS, если он не встроен в систему

## Первичная настройка

```bash
cargo run -p nime-client -- init \
  --network nime \
  --innernet-network nimenet \
  --dns-server 10.42.0.1:5354 \
  --dns-zone nime.internal \
  --innernet-bin /absolute/path/to/innernet
```

После этого в конфиг записываются:

- имя клиента
- имя `innernet` сети
- адрес DNS
- зона
- путь к `innernet`

## Первое подключение

```bash
sudo ./target/release/nime-client setup --invite invitation.toml
```

Команда делает:

- `innernet install`
- `innernet up`
- `innernet fetch`
- установку DNS-конфига

## Ежедневные команды

Показать сервисы:

```bash
cargo run -p nime-client -- services
```

Проверить peer:

```bash
cargo run -p nime-client -- health --peer laptop --port 22
```

Проверить сервис:

```bash
cargo run -p nime-client -- health wiki.docs
```

Открыть сервис:

```bash
cargo run -p nime-client -- open wiki.docs --port 8181
```

Показать конфиг:

```bash
cargo run -p nime-client -- show-config
```

## После перезагрузки

```bash
sudo ./target/release/nime-client reconnect
```

Если команда пишет, что не найден `innernet`, проверь:

```bash
cargo run -p nime-client -- show-config
```

И убедись, что `innernet_binary` указывает на реальный абсолютный путь.

## Отключение

```bash
sudo ./target/release/nime-client disconnect
```

Если DNS надо оставить:

```bash
sudo ./target/release/nime-client disconnect --keep-dns
```

## Автозапуск клиента

macOS:

```bash
sudo ./target/release/nime-client autostart-install --format launchd
```

Linux user service:

```bash
./target/release/nime-client autostart-install --format systemd-user
```
