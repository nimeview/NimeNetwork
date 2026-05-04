# Релизы И Упаковка

## Зачем нужны релизы

Пока проект можно использовать прямо из исходников, но release-архивы позволяют:

- не таскать весь репозиторий на прод
- раздавать клиентский пакет отдельно
- быстрее ставить серверный набор

## Сборка release-пакетов

```bash
scripts/package-release.sh
```

С версией:

```bash
scripts/package-release.sh --version 0.1.1
```

Если сборка уже сделана:

```bash
scripts/package-release.sh --skip-build
```

## Что входит в клиентский архив

- `nime-client`
- `innernet`
- quickstart
- user guide
- install script

## Что входит в серверный архив

- `nime-dns`
- `nime-portal`
- `nime-registry`
- `innernet-server`
- server helper script
- admin docs
- runbook
- install script

## Что ещё можно улучшить

- checksums
- version stamping из git tag
- CI release workflow
- автоматическая мультиплатформенная упаковка
