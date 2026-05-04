# Сервисы Nime

Это отдельный слой поверх сети, а не часть самой mesh-логики.

Сейчас здесь есть 3 минимальных сервиса:

- `wiki` — общая внутренняя страница и текстовая база знаний
- `storage` — общее файловое хранилище
- `ledger` — внутренняя игровая валюта сети с ограниченным выпуском и без связи с реальными деньгами

## Быстрый запуск

```bash
services/run-all.sh start
services/run-all.sh status
```

Локальные порты по умолчанию:

- `wiki` -> `0.0.0.0:8181`
- `storage` -> `0.0.0.0:8282`
- `ledger` -> `0.0.0.0:8383`

Остановка:

```bash
services/run-all.sh stop
```

## Регистрация в сети

После запуска сервисов зарегистрируй их в `nime-registry`:

```bash
services/register-defaults.sh --server-ip 10.42.0.1
```

Это создаст:

- `wiki.docs.nime.internal`
- `drive.apps.nime.internal`
- `ledger.apps.nime.internal`

## По сервисам

### Wiki

- главная страница: `GET /`
- raw текст: `GET /raw`
- сохранить текст: `POST /page`
- health: `GET /health`

### Storage

- список файлов: `GET /`
- скачать файл: `GET /files/<name>`
- загрузить файл: `POST /upload?name=file.zip`
- health: `GET /health`

Пример загрузки:

```bash
curl --data-binary @archive.zip http://127.0.0.1:8282/upload?name=archive.zip
```

### Ledger

- UI: `GET /`
- JSON API: `GET /api/ledger`
- выпуск монет: `POST /issue?account=community&amount=100`
- обновить описание: `POST /note`
- health: `GET /health`

Это не банк и не платёжная система. Это просто внутренний учёт ограниченной сетевой валюты без внешней стоимости.
