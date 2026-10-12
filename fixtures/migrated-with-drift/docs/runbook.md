# Runbook

## Environments

| Environment | Address |
|---|---|
| staging | `staging.parceltrack.example` |
| production | `api.parceltrack.example` |

## Starting and stopping

Run `parceltrack serve` under a process supervisor. The listener binds the
loopback interface only; put a reverse proxy in front of it to expose it. The
proxy terminates TLS with a certificate issued by Let's Encrypt and renewed by
certbot every 60 days.

## Backups

A cron job at 02:00 runs `scripts/backup.sh`, which copies the parcel data to
the backup bucket. Check the job's mail on Monday mornings.

## Settings

Local development keeps its settings in `.env` in the repository root.

## Who to call

Page the on-call owner listed in `.github/CODEOWNERS`.
