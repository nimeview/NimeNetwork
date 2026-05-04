# Nime Runbook

## Где создаётся `invitation.toml`

`innernet-server add-peer <interface>` не кладёт invitation в скрытый системный путь автоматически.
Команда спрашивает, куда сохранить файл.

Типичный prompt:

```text
Save peer invitation file to
```

Стандартное имя:

```text
<peer-name>.toml
```

Пример:

```bash
sudo server/innernet/target/release/innernet-server add-peer nimenet
```

Если ввести:

```text
arthur-client.toml
```

то файл будет создан в текущей директории shell, из которой ты запустил команду.

Если нужен фиксированный путь, укажи полный путь, например:

```text
/root/invitations/arthur-client.toml
```

После создания peer `innernet` печатает:

```text
Peer invitation file written to <path>
```

Именно этот файл потом надо передать на клиентскую машину.

## Запуск сервера на Linux

Предположения:

- путь к репозиторию: `~/NimeNetwork`
- имя сети / интерфейса: `nimenet`
- DNS-зона: `nime.internal`
- mesh IP сервера: `10.42.0.1`
- портал слушает на `8080`

### 1. Установить зависимости

```bash
cd ~/NimeNetwork
scripts/nime-server-linux.sh install-deps
```

Если Rust ещё не установлен:

```bash
curl https://sh.rustup.rs -sSf | sh
source "$HOME/.cargo/env"
```

### 2. Собрать бинарники

```bash
cd ~/NimeNetwork
scripts/nime-server-linux.sh build
```

### 3. Создать сеть `innernet`

```bash
cd ~/NimeNetwork/server/innernet
sudo target/release/innernet-server new
```

Если сеть уже создана, этот шаг можно пропустить.

Типовые ответы мастера:

- `Network name`: `nimenet`
- `Network CIDR`: `10.42.0.0/16`
- `Listen port`: `51820`
- `External endpoint`: внешний IP или домен сервера с этим портом

После завершения команда печатает путь к базе. Например:

```text
/var/lib/innernet-server/nimenet.db
```

### 4. Создать пользовательский CIDR

```bash
cd ~/NimeNetwork/server/innernet
sudo target/release/innernet-server add-cidr nimenet
```

Типовой вариант:

- `Parent CIDR`: `nimenet`
- `Name`: `folks`
- `CIDR`: `10.42.1.0/24`

Без этого `add-peer` позже не сможет предложить нормальный список CIDR для клиента.

### 5. Запустить `innernet-server`

В foreground:

```bash
cd ~/NimeNetwork/server/innernet
sudo target/release/innernet-server serve nimenet
```

В background:

```bash
cd ~/NimeNetwork/server/innernet
nohup sudo target/release/innernet-server serve nimenet > /root/innernet-server.log 2>&1 &
```

Проверка:

```bash
ps aux | grep innernet-server
tail -f /root/innernet-server.log
```

### 6. Создать invitation для клиента

```bash
cd ~/NimeNetwork/server/innernet
sudo target/release/innernet-server add-peer nimenet
```

Во время мастера обычно выбираются:

- `Eligible CIDRs for peer`: `folks`
- `IP`: например `10.42.1.1`
- `Name`: например `arthur-mac`
- `Make peer admin?`: по необходимости
- `Invite expires after`: например `14d`

На вопрос:

```text
Save peer invitation file to
```

можно указать:

```text
arthur-client.toml
```

или полный путь:

```text
/root/invitations/arthur-client.toml
```

Потом этот файл надо передать клиенту.

### 7. Забутстрапить registry

```bash
cd ~/NimeNetwork
scripts/nime-server-linux.sh bootstrap-registry \
  --portal-ip 10.42.0.1 \
  --portal-port 8080 \
  --owner arthur
```

### 8. Запустить портал

```bash
cd ~/NimeNetwork
scripts/nime-server-linux.sh run-portal
```

### 9. Запустить API registry

```bash
cd ~/NimeNetwork
scripts/nime-server-linux.sh run-registry-api --bind 0.0.0.0:4172
```

### 10. Запустить DNS

Используй реальный путь к базе `innernet`, который был напечатан при `innernet-server new`.

Пример:

```bash
cd ~/NimeNetwork
scripts/nime-server-linux.sh run-dns \
  --innernet-db /var/lib/innernet-server/nimenet.db
```

### 11. Сгенерировать systemd unit-файлы

```bash
cd ~/NimeNetwork
scripts/nime-server-linux.sh write-units \
  --innernet-db /var/lib/innernet-server/nimenet.db
```

Будут созданы:

- `deploy/systemd/nime-dns.service`
- `deploy/systemd/nime-portal.service`
- `deploy/systemd/nime-registry-api.service`

### 12. Установить unit-файлы одной командой

```bash
cd ~/NimeNetwork
scripts/nime-server-linux.sh install-units \
  --innernet-db /var/lib/innernet-server/nimenet.db
```

Для самого `innernet-server` также желательно включить systemd unit:

```bash
sudo systemctl enable --now innernet-server@nimenet
```

## Запуск клиента на macOS

Предположения:

- путь к репозиторию: `/Users/arthur/Documents/Project/NimeNetwork`
- invitation уже скопирован на Mac
- имя сети / интерфейса: `nimenet`
- DNS-сервер: `10.42.0.1:5354`

### 1. Собрать клиентский `innernet`

```bash
cd /Users/arthur/Documents/Project/NimeNetwork/server/innernet
cargo build --release --bin innernet
```

Ожидаемый бинарник:

```text
/Users/arthur/Documents/Project/NimeNetwork/server/innernet/target/release/innernet
```

### 2. Инициализировать `nime-client`

```bash
cd /Users/arthur/Documents/Project/NimeNetwork
cargo run -p nime-client -- init \
  --network nime \
  --innernet-network nimenet \
  --dns-server 10.42.0.1:5354 \
  --dns-zone nime.internal \
  --innernet-bin /Users/arthur/Documents/Project/NimeNetwork/server/innernet/target/release/innernet
```

Если `wireguard-go` уже стоит и виден в `PATH`, `nime-client` найдёт его сам.
`--wg-userspace-bin` нужен только если путь надо указать явно.

### 3. Проверить конфиг клиента

```bash
cd /Users/arthur/Documents/Project/NimeNetwork
cargo run -p nime-client -- doctor
```

### 4. Выполнить полную настройку клиента

Команда делает:

- `innernet install`
- `innernet up`
- `innernet fetch`
- установку DNS-конфига

```bash
cd /Users/arthur/Documents/Project/NimeNetwork
cargo build --release -p nime-client
sudo ./target/release/nime-client setup --invite invitation.toml
```

Если файл invitation лежит не в текущей директории, передай полный путь:

```bash
sudo ./target/release/nime-client setup --invite /full/path/to/arthur-client.toml
```

### 5. При необходимости установить DNS вручную

```bash
cd /Users/arthur/Documents/Project/NimeNetwork
sudo ./target/release/nime-client dns-install --format resolver
```

### 6. Переподключение после рестарта

```bash
cd /Users/arthur/Documents/Project/NimeNetwork
sudo ./target/release/nime-client reconnect
```

### 7. Отключение от сети

```bash
cd /Users/arthur/Documents/Project/NimeNetwork
sudo ./target/release/nime-client disconnect
```

Если DNS-конфиг нужно сохранить:

```bash
sudo ./target/release/nime-client disconnect --keep-dns
```

### 8. Установить launchd автозапуск

Сначала посмотреть конфиг:

```bash
cd /Users/arthur/Documents/Project/NimeNetwork
cargo run -p nime-client -- autostart-print --format launchd
```

Потом установить:

```bash
cd /Users/arthur/Documents/Project/NimeNetwork
sudo ./target/release/nime-client autostart-install --format launchd
```

### 9. Проверить peers

```bash
cd /Users/arthur/Documents/Project/NimeNetwork
sudo ./target/release/nime-client peers --tree
```

### 10. Посмотреть список сервисов

```bash
cd /Users/arthur/Documents/Project/NimeNetwork
cargo run -p nime-client -- services
```

JSON-вывод:

```bash
cargo run -p nime-client -- services --json
```

### 12. Сделать backup состояния сети

```bash
cd ~/NimeNetwork
scripts/nime-server-linux.sh backup-state \
  --innernet-db /var/lib/innernet-server/nimenet.db
```

### 13. Восстановить состояние сети из backup

```bash
cd ~/NimeNetwork
scripts/nime-server-linux.sh restore-state \
  --backup-dir backups/20260412-120000 \
  --innernet-db /var/lib/innernet-server/nimenet.db
```

## Что открывать в браузере

Когда клиент уже подключён и DNS настроен:

- `http://portal.nime.internal:8080`
- `http://wiki.nime.internal`

## Быстрая диагностика

### DNS не стартует и пишет `unable to open database file`

Использован placeholder-путь, а не реальный путь к базе `innernet`.

Нужный путь печатается командой:

```bash
sudo target/release/innernet-server new
```

Пример:

```text
/var/lib/innernet-server/nimenet.db
```

### `nime-client setup` пишет `failed to execute innernet`

Значит на клиенте нет корректного пути к бинарнику `innernet`.

Сначала собери его:

```bash
cd /Users/arthur/Documents/Project/NimeNetwork/server/innernet
cargo build --release --bin innernet
```

Потом снова вызови `nime-client init` с полным путём до бинарника.

### `innernet` на macOS пишет `Cannot find "wireguard-go"`

Установи `wireguard-go`.
В большинстве случаев `nime-client` найдёт его автоматически.
Если бинарник лежит в нестандартном месте, передай путь через `--wg-userspace-bin` при `init`.

### `innernet-server serve <interface>` даёт shell error

Не надо писать `<interface>` буквально.

Нужна реальная команда:

```bash
sudo target/release/innernet-server serve nimenet
```
