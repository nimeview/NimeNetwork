# DNS И Имена

## Базовая зона

Сейчас проект использует одну внутреннюю зону:

```text
nime.internal
```

## Имена peers

Peers приходят из `innernet.db` и публикуются как:

```text
<peer>.nime.internal
```

Пример:

```text
nimenet.nime.internal
```

## Имена сервисов

Канонический вид:

```text
<service>.<group>.nime.internal
```

Примеры:

- `portal.core.nime.internal`
- `wiki.docs.nime.internal`
- `drive.apps.nime.internal`
- `ledger.apps.nime.internal`

## Короткие алиасы

Если имя сервиса уникально во всей зоне, `nime-dns` дополнительно публикует:

```text
<service>.nime.internal
```

Но это только алиас. Надёжнее всегда использовать полное имя с группой.

## Почему не делать фейковые TLD

Имена вроде:

- `example.nime`
- `example.os`

смотрятся красиво, но неудобны как базовая архитектура, потому что:

- могут конфликтовать позже
- хуже сочетаются с реальными резолверами
- усложняют перенос проекта

Поэтому проект строится на одной canonical zone, а группы живут внутри неё.

## Как проверить DNS

Явный запрос к серверу DNS:

```bash
dig @10.42.0.1 -p 5354 wiki.docs.nime.internal
```

Через клиент:

```bash
cargo run -p nime-client -- resolve wiki.docs
```

## Что делает `nime-dns`

- читает peers из `innernet.db`
- читает сервисы через `nime-registry API`
- отвечает `A`-записями
- умеет reload без рестарта
