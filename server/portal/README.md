# nime-portal

Внутренняя стартовая страница и поиск по сервисам Nime.

## Запуск

```bash
cargo run -p nime-portal -- \
  --bind 127.0.0.1:8080 \
  --registry-api 127.0.0.1:4172 \
  --dns-zone nime.internal
```

Потом открыть:

```text
http://127.0.0.1:8080/
```

Пример поиска:

```text
http://127.0.0.1:8080/?q=portal
```

Проверка health:

```text
http://127.0.0.1:8080/health
```
