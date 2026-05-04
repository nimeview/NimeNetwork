# Диагностика И Типовые Проблемы

## `nime-client services` пишет timeout к `10.42.0.1:4172`

Скорее всего:

- не запущен `nime-registry API`
- не поднят `innernet-server`
- клиентский интерфейс не переподнят после перезагрузки

Проверка на клиенте:

```bash
sudo ./target/release/nime-client peers --tree
sudo ./target/release/nime-client reconnect
```

Проверка на сервере:

```bash
ps aux | grep innernet-server
ps aux | grep nime-registry
```

## `innernet-server` не запускается через `sudo`

Если в логе:

```text
sudo: server/innernet/target/release/innernet-server: command not found
```

значит путь был указан без `./`.

Правильно:

```bash
sudo ./server/innernet/target/release/innernet-server serve nimenet
```

## `nime-client reconnect` не находит `innernet`

Проверь:

```bash
cargo run -p nime-client -- show-config
```

Если `innernet_binary=innernet`, а бинарник не лежит в `PATH`, переинициализируй клиент с абсолютным путём.

## `dig portal...` идёт в 1.1.1.1

Это не обязательно ошибка самой сети. `dig` по умолчанию может идти в системный внешний resolver.

Для проверки внутреннего DNS лучше явно указывать сервер:

```bash
dig @10.42.0.1 -p 5354 portal.core.nime.internal
```

## Имя сервиса не открывается, хотя DNS резолвится

Значит чаще всего:

- сервис не слушает нужный порт
- bind address не тот
- приложение поднято только на `127.0.0.1`
- неверный порт в registry

Проверь на сервере:

```bash
curl http://127.0.0.1:<port>/health
```

## После перезагрузки сервера всё “сломалось”

Обычно это значит, что:

- `innernet-server` не включён в автозапуск
- `nime-*` сервисы не поставлены как `systemd` units

Решение:

```bash
scripts/nime-server-linux.sh install-units --innernet-db /var/lib/innernet-server/nimenet.db
sudo systemctl enable --now innernet-server@nimenet
```
