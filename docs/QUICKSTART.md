# Быстрый Старт

Это самый короткий рабочий путь для запуска Nime.

## Сервер

Предположения:

- mesh IP сервера: `10.42.0.1`
- DNS-зона: `nime.internal`
- путь к репозиторию: `~/NimeNetwork`

Установить зависимости и собрать проект:

```bash
cd ~/NimeNetwork
scripts/nime-server-linux.sh install-deps
scripts/nime-server-linux.sh build
```

Создать сеть один раз:

```bash
cd ~/NimeNetwork/server/innernet
sudo target/release/innernet-server new
sudo target/release/innernet-server add-cidr nimenet
```

Запустить сервер:

```bash
cd ~/NimeNetwork/server/innernet
sudo target/release/innernet-server serve nimenet
```

Создать invitation:

```bash
cd ~/NimeNetwork/server/innernet
sudo target/release/innernet-server add-peer nimenet
```

Поднять базовые сервисы Nime:

```bash
cd ~/NimeNetwork
scripts/nime-server-linux.sh bootstrap-registry \
  --portal-ip 10.42.0.1 \
  --portal-port 8080 \
  --owner arthur

scripts/nime-server-linux.sh run-registry-api --bind 0.0.0.0:4172

scripts/nime-server-linux.sh run-dns \
  --innernet-db /var/lib/innernet-server/nimenet.db

scripts/nime-server-linux.sh run-portal
```

## Клиент

Собрать `innernet` один раз:

```bash
cd ~/NimeNetwork/server/innernet
cargo build --release --bin innernet
```

Инициализировать `nime-client`:

```bash
cd ~/NimeNetwork
cargo run -p nime-client -- init \
  --network nime \
  --innernet-network nimenet \
  --dns-server 10.42.0.1:5354 \
  --dns-zone nime.internal \
  --innernet-bin "$HOME/NimeNetwork/server/innernet/target/release/innernet"
```

Подключиться:

```bash
cd ~/NimeNetwork
cargo build --release -p nime-client
sudo ./target/release/nime-client setup --invite invitation.toml
```

Проверить:

```bash
cargo run -p nime-client -- resolve portal
cargo run -p nime-client -- services
cargo run -p nime-client -- services --json
cargo run -p nime-client -- health portal --port 8080
cargo run -p nime-client -- open portal --port 8080
```

## После перезагрузки

Переподключение:

```bash
sudo ./target/release/nime-client reconnect
```

Отключение:

```bash
sudo ./target/release/nime-client disconnect
```

Автозапуск клиента:

```bash
sudo ./target/release/nime-client autostart-install --format launchd
```

Генерация server unit-файлов:

```bash
cd ~/NimeNetwork
scripts/nime-server-linux.sh write-units \
  --innernet-db /var/lib/innernet-server/nimenet.db
```

Или сразу установка и включение:

```bash
cd ~/NimeNetwork
scripts/nime-server-linux.sh install-units \
  --innernet-db /var/lib/innernet-server/nimenet.db
```

Backup состояния:

```bash
cd ~/NimeNetwork
scripts/nime-server-linux.sh backup-state \
  --innernet-db /var/lib/innernet-server/nimenet.db
```
