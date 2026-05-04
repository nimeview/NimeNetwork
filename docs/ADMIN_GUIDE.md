# Гайд Для Администратора

Этот документ для того, кто поднимает сервер сети, выдаёт invitation-файлы и публикует сервисы.

## Что делает администратор

- поднимает `innernet-server`
- создаёт CIDR и peers
- запускает `nime-registry`, `nime-dns`, `nime-portal`
- публикует сервисы
- делает backup состояния

## Базовый серверный цикл

Собрать всё:

```bash
cd ~/NimeNetwork
scripts/nime-server-linux.sh build
```

Поднять registry, API, DNS и portal:

```bash
scripts/nime-server-linux.sh bootstrap-registry \
  --portal-ip 10.42.0.1 \
  --portal-port 8080 \
  --owner arthur

scripts/nime-server-linux.sh run-registry-api --bind 0.0.0.0:4172
scripts/nime-server-linux.sh run-dns --innernet-db /var/lib/innernet-server/nimenet.db
scripts/nime-server-linux.sh run-portal
```

Установить `systemd` units:

```bash
scripts/nime-server-linux.sh install-units \
  --innernet-db /var/lib/innernet-server/nimenet.db
```

## Выдать invitation пользователю

```bash
cd ~/NimeNetwork/server/innernet
sudo target/release/innernet-server add-peer nimenet
```

## Опубликовать сервис

```bash
cargo run -p nime-registry -- add \
  --db server/registry/nime-registry.db \
  --name wiki \
  --target-ip 10.42.0.1 \
  --port 8081 \
  --proto http \
  --title "Wiki" \
  --description "Internal docs" \
  --owner arthur \
  --tags docs
```

Обновить сервис:

```bash
cargo run -p nime-registry -- update \
  --db server/registry/nime-registry.db \
  --name wiki \
  --port 8082 \
  --description "Moved to a new port"
```

## Backup и restore

Backup:

```bash
scripts/nime-server-linux.sh backup-state \
  --innernet-db /var/lib/innernet-server/nimenet.db
```

Restore:

```bash
scripts/nime-server-linux.sh restore-state \
  --backup-dir backups/20260412-120000 \
  --innernet-db /var/lib/innernet-server/nimenet.db
```
