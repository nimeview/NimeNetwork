# Релизные Бинарники

Сейчас в проекте есть локальный release workflow без CI.

## Что умеет packaging

Скрипт:

```bash
scripts/package-release.sh
```

делает следующее:

- собирает release-бинарники workspace
- собирает vendored `innernet` и `innernet-server`
- формирует отдельный пакет клиента
- формирует отдельный пакет сервера
- упаковывает оба в `.tar.gz`

## Что получается на выходе

В `dist/` появятся архивы примерно такого вида:

```text
dist/nime-client-0.1.0-x86_64-unknown-linux-gnu.tar.gz
dist/nime-server-0.1.0-x86_64-unknown-linux-gnu.tar.gz
```

## Клиентский пакет

Внутри:

- `nime-client`
- `innernet`
- `USER_GUIDE.md`
- `QUICKSTART.md`
- `install-client.sh`

## Серверный пакет

Внутри:

- `nime-dns`
- `nime-portal`
- `nime-registry`
- `innernet-server`
- `nime-server-linux.sh`
- `ADMIN_GUIDE.md`
- `RUNBOOK.md`
- `install-server.sh`

## Сборка

Обычный запуск:

```bash
cd ~/NimeNetwork
scripts/package-release.sh
```

С кастомной версией:

```bash
scripts/package-release.sh --version 0.1.1
```

Если release-бинарники уже собраны:

```bash
scripts/package-release.sh --skip-build
```

## Ограничения текущего подхода

- пакеты собираются под текущую хост-платформу
- это ещё не multi-platform release pipeline
- нет подписей, checksums и GitHub Release workflow
- install-скрипты пока минимальные

## Что логично делать дальше

- checksums
- version stamping из git tag
- GitHub Actions / CI release
- отдельные пакеты под macOS и Linux
- более умные install-скрипты для client/server
