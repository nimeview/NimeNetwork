# nime-registry

Реестр сервисов для сети Nime.

## Основные команды

Инициализировать базу:

```bash
cargo run -p nime-registry -- init-db
```

Быстрый bootstrap портала:

```bash
cargo run -p nime-registry -- bootstrap \
  --portal-ip 10.42.0.1 \
  --portal-port 8080 \
  --owner arthur
```

Добавить сервис:

```bash
cargo run -p nime-registry -- add \
  --name portal \
  --target-ip 10.42.0.1 \
  --port 8080 \
  --proto http \
  --title "Portal" \
  --description "Shared start page" \
  --owner arthur \
  --tags infra,search
```

Обновить сервис без `remove/add`:

```bash
cargo run -p nime-registry -- update \
  --name portal \
  --port 3000 \
  --description "New port for portal"
```

Показать список сервисов:

```bash
cargo run -p nime-registry -- list
```

Показать один сервис:

```bash
cargo run -p nime-registry -- get --name portal
```

Удалить сервис:

```bash
cargo run -p nime-registry -- remove --name portal
```

Экспортировать имена для DNS:

```bash
cargo run -p nime-registry -- export-dns
```

Пример вывода:

```text
portal 10.42.0.1
wiki 10.42.1.20
```

## HTTP API

Запуск:

```bash
cargo run -p nime-registry -- serve --bind 0.0.0.0:4172
```

Маршруты:

- `GET /health`
- `GET /services`
- `GET /services?q=portal`
- `GET /services/portal`
