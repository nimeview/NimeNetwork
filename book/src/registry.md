# Registry И Публикация Сервисов

## Зачем нужен registry

Запуск приложения ещё не делает его частью сети.

Чтобы сервис появился в Nime, нужно:

1. Запустить приложение
2. Добавить его в `nime-registry`
3. Дождаться, пока `nime-dns` и `nime-portal` его увидят

## Минимальная модель сервиса

У сервиса есть:

- `name`
- `group`
- `target_ip`
- `port`
- `protocol`
- `title`
- `description`
- `owner`
- `tags`

## Добавление сервиса

Пример:

```bash
cd ~/NimeNetwork
cargo run -p nime-registry -- add \
  --db server/registry/nime-registry.db \
  --name blog \
  --group apps \
  --target-ip 10.42.0.1 \
  --port 3000 \
  --proto http \
  --title "Blog" \
  --description "Внутренний блог" \
  --owner nime \
  --tags web,shared
```

Это создаст каноническое имя:

```text
blog.apps.nime.internal
```

## Обновление сервиса

```bash
cargo run -p nime-registry -- update \
  --db server/registry/nime-registry.db \
  --name blog \
  --port 4000 \
  --description "Новый порт"
```

Можно менять:

- группу
- IP
- порт
- title
- owner
- tags

## Просмотр сервисов

```bash
cargo run -p nime-registry -- list
cargo run -p nime-registry -- get --name blog
```

## Registry API

Запуск:

```bash
cargo run -p nime-registry -- serve --bind 0.0.0.0:4172
```

Endpoints:

- `GET /health`
- `GET /services`
- `GET /services?q=wiki`
- `GET /services/<name>`

## Практические правила

- используй mesh IP, а не публичный IP
- не регистрируй сервис, пока он реально не слушает порт
- owner для встроенных сервисов можно держать как `nime`
- короткие и стабильные имена предпочтительнее

## Рекомендуемые группы

- `core`
- `apps`
- `docs`
- `infra`
- `media`
- `dev`
- `personal`
