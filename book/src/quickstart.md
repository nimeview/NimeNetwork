# Быстрый Старт

Ниже самый короткий рабочий путь для запуска сети и первого клиента.

## Сервер

Предположения:

- репозиторий: `~/NimeNetwork`
- имя сети: `nimenet`
- mesh IP сервера: `10.42.0.1`
- зона DNS: `nime.internal`

### 1. Установить зависимости и собрать проект

```bash
cd ~/NimeNetwork
scripts/nime-server-linux.sh install-deps
scripts/nime-server-linux.sh build
```

### 2. Создать сеть `innernet`

```bash
cd ~/NimeNetwork/server/innernet
sudo target/release/innernet-server new
```

Во время мастера задай хотя бы:

- `Network name`: `nimenet`
- `Network CIDR`: `10.42.0.0/16`
- `Listen port`: `51820`
- `External endpoint`: внешний адрес сервера

После этого запомни путь к базе, например:

```text
/var/lib/innernet-server/nimenet.db
```

### 3. Создать пользовательский CIDR

```bash
cd ~/NimeNetwork/server/innernet
sudo target/release/innernet-server add-cidr nimenet
```

Типовой вариант:

- parent: `nimenet`
- name: `folks`
- cidr: `10.42.1.0/24`

### 4. Запустить сервер сети

```bash
cd ~/NimeNetwork
sudo ./server/innernet/target/release/innernet-server serve nimenet
```

### 5. Создать invitation

```bash
cd ~/NimeNetwork/server/innernet
sudo target/release/innernet-server add-peer nimenet
```

При создании peer:

- выбери CIDR `folks`
- укажи имя клиента
- сохрани invitation в `.toml` файл

### 6. Запустить registry, DNS и portal

```bash
cd ~/NimeNetwork
scripts/nime-server-linux.sh bootstrap-registry \
  --portal-ip 10.42.0.1 \
  --portal-port 8080 \
  --owner nime

cargo run -p nime-registry -- serve --bind 0.0.0.0:4172
cargo run -p nime-dns -- --bind 0.0.0.0:5354 --zone nime.internal --innernet-db /var/lib/innernet-server/nimenet.db --registry-api 127.0.0.1:4172
cargo run -p nime-portal -- --bind 0.0.0.0:8080 --registry-api 127.0.0.1:4172 --dns-zone nime.internal
```

## Клиент

Предположения:

- репозиторий: `/Users/arthur/Documents/Project/NimeNetwork`
- invitation уже скопирован на клиентскую машину

### 1. Собрать `innernet`

```bash
cd /Users/arthur/Documents/Project/NimeNetwork/server/innernet
cargo build --release --bin innernet
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

### 3. Подключиться

```bash
cd /Users/arthur/Documents/Project/NimeNetwork
cargo build --release -p nime-client
sudo ./target/release/nime-client setup --invite invitation.toml
```

### 4. Проверить

```bash
cargo run -p nime-client -- services
cargo run -p nime-client -- open portal.core --port 8080
```
