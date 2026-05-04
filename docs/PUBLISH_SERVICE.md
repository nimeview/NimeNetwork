# Публикация Сервиса

Этот гайд для сценария: «я написал сайт или приложение, как теперь сделать его доступным внутри Nime?»

## Базовая модель

Чтобы опубликовать сервис, нужны 3 вещи:

1. Запущенное приложение, которое слушает IP и порт
2. Запись в `nime-registry`
3. DNS-имя, которое раздаёт `nime-dns`

`nime-dns` читает сервисы из `nime-registry`, поэтому после регистрации имя появляется автоматически.

Теперь у сервиса есть не только имя, но и группа. Канонический DNS-вид такой:

- `<service>.<group>.nime.internal`

Например:

- `blog.apps.nime.internal`
- `wiki.docs.nime.internal`
- `portal.core.nime.internal`

Если имя сервиса уникально во всей зоне, `nime-dns` также создаёт короткий алиас:

- `blog.nime.internal`

Если одинаковое имя появится в двух группах, короткий алиас исчезнет, а канонические имена останутся.

## Вариант 1: сервис на Linux-сервере

Пример:

- приложение работает на сервере Nime
- mesh IP сервера: `10.42.0.1`
- порт приложения: `3000`
- имя сервиса: `blog`
- группа: `apps`

Запусти приложение так, чтобы оно было доступно из mesh-сети. Например:

```bash
python3 -m http.server 3000 --bind 0.0.0.0
```

Зарегистрируй сервис:

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
  --description "Internal blog" \
  --owner arthur \
  --tags web,docs
```

Подожди один цикл reload у DNS или перезапусти `nime-dns`.

Проверка с клиента:

```bash
cargo run -p nime-client -- resolve blog
cargo run -p nime-client -- open blog --port 3000
```

Результат:

- каноническое DNS-имя: `blog.apps.nime.internal`
- короткий алиас: `blog.nime.internal`
- URL: `http://blog.apps.nime.internal:3000`

## Вариант 2: сервис на другом peer

Пример:

- peer: `workstation`
- peer IP: `10.42.1.20`
- порт приложения: `5173`
- имя сервиса: `dashboard`
- группа: `apps`

Запусти приложение на этом peer так, чтобы оно слушало доступный адрес.

Зарегистрируй сервис на сервере:

```bash
cd ~/NimeNetwork
cargo run -p nime-registry -- add \
  --db server/registry/nime-registry.db \
  --name dashboard \
  --group apps \
  --target-ip 10.42.1.20 \
  --port 5173 \
  --proto http \
  --title "Dashboard" \
  --description "Internal dashboard" \
  --owner arthur \
  --tags web,admin
```

После этого клиенты могут открывать:

```text
http://dashboard.apps.nime.internal:5173
```

## Обновление сервиса

Теперь не нужно делать `remove/add`, можно использовать `update`.

Например, если сменился IP или порт:

```bash
cargo run -p nime-registry -- update \
  --db server/registry/nime-registry.db \
  --name dashboard \
  --group dashboards \
  --target-ip 10.42.1.21 \
  --port 8080 \
  --description "Internal dashboard on the new port"
```

## Практические правила

- Используй mesh IP, а не публичный IP
- Регистрируй реальный порт приложения
- Если сервис не на `80` или `443`, указывай порт в URL
- Если имя резолвится, но страница не открывается, проблема обычно в bind address, firewall или неправильном порте

## Рекомендуемые группы

- `core` — системные сервисы сети
- `apps` — обычные пользовательские приложения
- `docs` — wiki, документация, knowledge base
- `infra` — мониторинг, логи, админка
- `media` — галереи, музыка, видео
- `dev` — CI, git, registry, песочницы
- `personal` — личные сервисы участников

Имена сервисов лучше делать короткими и стабильными, а группу выбирать по смыслу, потому что вместе они становятся DNS-именем.
