# Встроенные Сервисы

В проекте есть отдельная папка `services/` для прикладных сервисов сети.

Это важно: они не встроены в саму сетевую логику, а живут отдельно поверх неё.

## Что уже есть

### Wiki

Сервис:

- crate: `nime-wiki`
- порт: `8181`
- DNS: `wiki.docs.nime.internal`

Назначение:

- общая страница сети
- документация
- заметки и правила

### Storage

Сервис:

- crate: `nime-storage`
- порт: `8282`
- DNS: `drive.apps.nime.internal`

Назначение:

- общее файловое хранилище
- архивы, документы, установщики

### Ledger

Сервис:

- crate: `nime-ledger`
- порт: `8383`
- DNS: `ledger.apps.nime.internal`

Назначение:

- внутренняя игровая валюта сети
- ограниченный выпуск
- без связи с реальными деньгами
- без внешней стоимости

Это не банк и не платёжная система.

## Запуск всех встроенных сервисов

```bash
cd ~/NimeNetwork
services/run-all.sh start
services/run-all.sh status
```

Остановка:

```bash
services/run-all.sh stop
```

## Регистрация встроенных сервисов

```bash
cd ~/NimeNetwork
services/register-defaults.sh --server-ip 10.42.0.1
```

Это создаст записи в registry для:

- `wiki.docs`
- `drive.apps`
- `ledger.apps`

## Где лежат данные

- wiki: `services/data/wiki`
- storage: `services/data/storage`
- ledger: `services/data/ledger/state.json`

## Как открыть

Через браузер:

- `http://wiki.docs.nime.internal:8181`
- `http://drive.apps.nime.internal:8282`
- `http://ledger.apps.nime.internal:8383`

Через клиент:

```bash
cargo run -p nime-client -- open wiki.docs --port 8181
cargo run -p nime-client -- open drive.apps --port 8282
cargo run -p nime-client -- open ledger.apps --port 8383
```
