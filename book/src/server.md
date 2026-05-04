# Сервер

Этот раздел про Linux-машину, где живёт сеть.

## Что держит сервер

Сервер обычно хранит:

- `innernet-server`
- `nime-registry`
- `nime-dns`
- `nime-portal`
- встроенные сервисы из `services/`

## Развёртывание `innernet-server` с нуля

Этот сценарий нужен для полностью чистого Linux-сервера, где ещё нет готовой сети.

Предположения:

- репозиторий лежит в `~/NimeNetwork`
- имя сети будет `nimenet`
- mesh-сеть будет `10.42.0.0/16`
- пользовательский CIDR будет `10.42.1.0/24`
- внешний endpoint сервера будет доступен клиентам

### 1. Установить зависимости и собрать бинарники

```bash
cd ~/NimeNetwork
scripts/nime-server-linux.sh install-deps
scripts/nime-server-linux.sh build
```

### 2. Создать сеть

```bash
cd ~/NimeNetwork/server/innernet
sudo target/release/innernet-server new
```

Во время мастера обычно вводятся такие значения:

- `Network name`: `nimenet`
- `Network CIDR`: `10.42.0.0/16`
- `Listen port`: `51820`
- `External endpoint`: внешний IP или домен сервера, например `203.0.113.10:51820`

После этого `innernet-server` создаст базу и напечатает путь, обычно:

```text
/var/lib/innernet-server/nimenet.db
```

### 3. Создать пользовательский CIDR

Сразу после `new` сеть содержит только служебный серверный peer. Для обычных клиентов нужен отдельный CIDR.

```bash
cd ~/NimeNetwork/server/innernet
sudo target/release/innernet-server add-cidr nimenet
```

Типовые ответы:

- `Parent CIDR`: `nimenet`
- `Name`: `folks`
- `CIDR`: `10.42.1.0/24`

Если потом понадобятся отдельные сетевые зоны под сервисы или инфраструктуру, можно создавать новые CIDR отдельно.

### 4. Запустить сервер сети

В foreground:

```bash
cd ~/NimeNetwork
sudo ./server/innernet/target/release/innernet-server serve nimenet
```

В background:

```bash
cd ~/NimeNetwork
nohup sudo ./server/innernet/target/release/innernet-server serve nimenet > /root/innernet-server.log 2>&1 &
```

Проверка:

```bash
ps aux | grep innernet-server
tail -f /root/innernet-server.log
```

### 5. Создать первый peer и invitation

```bash
cd ~/NimeNetwork/server/innernet
sudo target/release/innernet-server add-peer nimenet
```

Обычно мастер спросит:

- `Eligible CIDRs for peer`
- `IP`
- `Name`
- `Make <peer> an admin?`
- `Invite expires after`
- `Save peer invitation file to`

Типовой пример:

- CIDR: `folks`
- IP: `10.42.1.1`
- Name: `arthur-mac`
- admin: `yes` или `no` по задаче
- expiration: `14d`
- invitation file: `/root/invitations/arthur-mac.toml`

Именно этот файл потом передаётся клиенту.

### 6. Настроить автозапуск `innernet-server`

Если сеть уже проверена вручную, лучше сразу включить systemd unit:

```bash
sudo systemctl enable --now innernet-server@nimenet
```

Проверка:

```bash
systemctl status innernet-server@nimenet
```

### 7. Только после этого поднимать `nime-*`

Когда `innernet-server` уже живой, можно переходить к:

- `nime-registry`
- `nime-dns`
- `nime-portal`
- сервисам из `services/`

## Где лежат важные данные

### База `innernet`

Обычно:

```text
/var/lib/innernet-server/nimenet.db
```

### База registry

Обычно:

```text
server/registry/nime-registry.db
```

### Данные встроенных сервисов

Обычно:

- `services/data/wiki/`
- `services/data/storage/`
- `services/data/ledger/`

## Ручной запуск основных компонентов

### `innernet-server`

Foreground:

```bash
cd ~/NimeNetwork
sudo ./server/innernet/target/release/innernet-server serve nimenet
```

Background:

```bash
cd ~/NimeNetwork
nohup sudo ./server/innernet/target/release/innernet-server serve nimenet > /root/innernet-server.log 2>&1 &
```

### `nime-registry API`

```bash
cd ~/NimeNetwork
cargo run -p nime-registry -- serve --bind 0.0.0.0:4172
```

### `nime-dns`

```bash
cd ~/NimeNetwork
cargo run -p nime-dns -- \
  --bind 0.0.0.0:5354 \
  --zone nime.internal \
  --innernet-db /var/lib/innernet-server/nimenet.db \
  --registry-api 127.0.0.1:4172
```

### `nime-portal`

```bash
cd ~/NimeNetwork
cargo run -p nime-portal -- \
  --bind 0.0.0.0:8080 \
  --registry-api 127.0.0.1:4172 \
  --dns-zone nime.internal
```

## Создание invitation

Invitation создаётся так:

```bash
cd ~/NimeNetwork/server/innernet
sudo target/release/innernet-server add-peer nimenet
```

Команда спросит, куда сохранить файл.

Например:

```text
/root/invitations/arthur-client.toml
```

Именно этот файл нужно передать клиенту.

## Systemd

Для `nime-*` уже есть helper:

```bash
cd ~/NimeNetwork
scripts/nime-server-linux.sh install-units \
  --innernet-db /var/lib/innernet-server/nimenet.db
```

Это ставит:

- `nime-registry-api.service`
- `nime-dns.service`
- `nime-portal.service`

`innernet-server` можно также включить отдельно:

```bash
sudo systemctl enable --now innernet-server@nimenet
```

## После перезагрузки сервера

Проверь:

```bash
ps aux | grep innernet-server
ps aux | grep nime-registry
ps aux | grep nime-dns
ps aux | grep nime-portal
```

Или через systemd:

```bash
systemctl status innernet-server@nimenet
systemctl status nime-registry-api
systemctl status nime-dns
systemctl status nime-portal
```
