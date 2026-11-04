# Security policy

## Supported versions

perfview is released from `main`. Only the most recent release receives fixes.

| Version        | Supported          |
| -------------- | ------------------ |
| Latest release | :white_check_mark: |
| Anything older | :x:                |

## What perfview does on your machine

perfview is a read-only observer. It is useful to know exactly what it touches
when judging the impact of a vulnerability:

- It **reads** `/proc`, `/sys` and the mount table.
- It **reads** NVIDIA telemetry through NVML, which it loads at runtime from
  your installed driver.
- It **writes** one small preferences file under your user configuration
  directory, typically `~/.config/perfview/`.
- It opens **no network connections**, sends **no telemetry**, and requires
  **no elevated privileges**. Readings that genuinely need root, such as the
  processor's energy counter, are shown as "Not available" rather than
  prompting for a password.
- It never writes to `/proc` or `/sys`, and it cannot change any system
  setting, clock speed, fan curve or power limit.

The crate is built with `unsafe_code = "forbid"`, so no unsafe Rust is compiled
from this repository.

## Reporting a vulnerability

Please **do not open a public issue** for a security problem.

Report it privately through GitHub's
[security advisory form](https://github.com/crxig-rxberts/perfview/security/advisories/new).

Please include:

- A description of the issue and why you believe it is a security problem
- Steps to reproduce it
- The perfview version and your distribution and kernel version
- Any proof-of-concept you have

You can expect an acknowledgement within **5 working days**, and an assessment
with a plan within **14 days**. If the report is valid you will be credited in
the advisory and the release notes, unless you would rather not be.

## Dependency security

Every push and pull request runs:

- **CodeQL** static analysis
- **`cargo deny`**, which checks the dependency tree for known advisories,
  disallowed licences, banned crates and untrusted sources

Dependency updates are proposed automatically by Renovate, using the shared
[`crxig-rxberts/renovate-config`](https://github.com/crxig-rxberts/renovate-config)
policy. Updates to GitHub Actions workflows always require human review.
