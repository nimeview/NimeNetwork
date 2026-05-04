# nime-client

CLI-клиент для локальной настройки и работы с сетью Nime.

## Основные команды

Инициализация конфига:

```bash
cargo run -p nime-client -- init \
  --network nime \
  --innernet-network nimenet \
  --dns-server 10.42.0.1:5354 \
  --dns-zone nime.internal \
  --innernet-bin /absolute/path/to/innernet
```

Если `wireguard-go` уже установлен и виден в `PATH`, `nime-client` найдёт его автоматически.
`--wg-userspace-bin` нужен только если ты хочешь явно зафиксировать путь.

Статус:

```bash
cargo run -p nime-client -- status
```

Диагностика:

```bash
cargo run -p nime-client -- doctor
```

Подключение по invitation:

```bash
sudo ./target/release/nime-client setup --invite invitation.toml
```

Переподключение после перезагрузки или обрыва:

```bash
sudo ./target/release/nime-client reconnect
```

Отключение от сети:

```bash
sudo ./target/release/nime-client disconnect
```

Если DNS-конфиг нужно оставить на месте:

```bash
sudo ./target/release/nime-client disconnect --keep-dns
```

Показать peers:

```bash
cargo run -p nime-client -- peers --tree
```

Обновить состояние:

```bash
cargo run -p nime-client -- fetch
```

Поднять интерфейс:

```bash
cargo run -p nime-client -- up
```

## DNS и быстрые проверки

Разрешить имя через внутренний DNS:

```bash
cargo run -p nime-client -- resolve portal
```

Показать сервисы из registry API:

```bash
cargo run -p nime-client -- services
```

С фильтром:

```bash
cargo run -p nime-client -- services portal
```

В JSON:

```bash
cargo run -p nime-client -- services --json
```

Проверить DNS и HTTP health одной командой:

```bash
cargo run -p nime-client -- health portal --port 8080
```

Если порт не указан, `nime-client` попытается взять его из registry API.

Проверить peer:

```bash
cargo run -p nime-client -- health --peer laptop --port 22
```

Открыть сервис в браузере:

```bash
cargo run -p nime-client -- open portal --port 8080
```

Вывести DNS-конфиг:

```bash
cargo run -p nime-client -- dns-print --format resolver
```

Установить DNS-конфиг:

```bash
cargo run -p nime-client -- dns-install --format resolver
```

## Автозапуск

Показать конфиг автозапуска:

```bash
cargo run -p nime-client -- autostart-print --format launchd
```

Установить автозапуск на macOS:

```bash
sudo ./target/release/nime-client autostart-install --format launchd
```

Установить автозапуск для Linux user session:

```bash
cargo run -p nime-client -- autostart-install --format systemd-user
```

## Конфиг

Путь по умолчанию: `client/nime-client.conf`

Основные поля:

- `innernet_binary` — путь до реального бинарника `innernet`
- `wg_userspace_implementation` — опционально, если автодетект не подходит
