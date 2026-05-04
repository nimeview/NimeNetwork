# Ежедневная Работа

## Для администратора

Типовой цикл:

1. Проверить, что сервер сети поднят
2. Проверить `registry`, `dns`, `portal`
3. При необходимости выдать invitation
4. Поднять прикладные сервисы
5. Зарегистрировать новые сервисы

Команды:

```bash
ps aux | grep innernet-server
ps aux | grep nime-registry
ps aux | grep nime-dns
ps aux | grep nime-portal
services/run-all.sh status
```

## Для обычного клиента

Типовой цикл:

1. Переподключиться после перезагрузки
2. Посмотреть список сервисов
3. Открыть нужный сервис

Команды:

```bash
sudo ./target/release/nime-client reconnect
cargo run -p nime-client -- services
cargo run -p nime-client -- open portal.core --port 8080
```

## Проверка состояния сети

Peers:

```bash
sudo ./target/release/nime-client peers --tree
```

DNS:

```bash
cargo run -p nime-client -- resolve wiki.docs
```

Health:

```bash
cargo run -p nime-client -- health wiki.docs
cargo run -p nime-client -- health --peer nimenet --port 22
```

## Когда что использовать

### `setup`

Только при первом подключении по invitation.

### `reconnect`

После перезагрузки клиента или после ручного обрыва.

### `disconnect`

Когда надо полностью отцепить клиента от сети.

### `services`

Когда нужно быстро понять, что вообще доступно в сети.

### `open`

Когда нужно открыть сервис в браузере без ручного набора URL.
