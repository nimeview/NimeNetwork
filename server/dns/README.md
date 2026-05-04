# nime-dns

Внутренний authoritative DNS-сервер для сети Nime.

## Что умеет сейчас

- слушает UDP
- обслуживает одну внутреннюю зону
- отвечает `A`-записями
- читает peers из базы `innernet`
- читает сервисы через HTTP API `nime-registry`
- выдаёт канонические сервисные имена вида `<service>.<group>.<zone>`
- создаёт короткий алиас `<service>.<zone>`, если имя сервиса уникально во всей зоне
- периодически перечитывает данные без рестарта

## Запуск

Базовый вариант:

```bash
cargo run -p nime-dns -- \
  --bind 0.0.0.0:5354 \
  --zone nime.internal \
  --innernet-db /var/lib/innernet-server/nimenet.db
```

С сервисами из registry:

```bash
cargo run -p nime-dns -- \
  --bind 0.0.0.0:5354 \
  --zone nime.internal \
  --innernet-db /var/lib/innernet-server/nimenet.db \
  --registry-api 127.0.0.1:4172
```

С кастомным интервалом reload:

```bash
cargo run -p nime-dns -- --reload-secs 10
```

## Схема имён

Peers:

- `peer-name.nime.internal`

Сервисы:

- `portal.core.nime.internal`
- `wiki.docs.nime.internal`
- `grafana.infra.nime.internal`

Если, например, `portal` существует только в одной группе, дополнительно будет работать:

- `portal.nime.internal`

Если одинаковое имя есть в нескольких группах, короткий алиас не создаётся, и остаются только канонические имена.

## Что логично делать дальше

- `AAAA`
- `SRV` и `TXT`
- более понятная диагностика отсутствующих имён
- метрики и health-aware логика
