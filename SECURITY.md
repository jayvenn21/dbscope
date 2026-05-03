# Security Policy

## Reporting a Vulnerability

If you discover a security vulnerability in dbscope, please report it responsibly.

Open a GitHub issue at https://github.com/jayvenn21/dbscope/issues with the label `security`.

Please include:

- Description of the vulnerability
- Steps to reproduce
- Potential impact
- Suggested fix (if any)

## Scope

dbscope is a **read-only** analysis tool. It connects to databases using the credentials you provide and only executes `SELECT` queries against metadata catalogs (`information_schema`, `pg_catalog`, `sqlite_master`, `system.*`).

dbscope does not:

- Modify database data or schema
- Store database credentials (they are passed via CLI flags or environment variables)
- Send any data to external services
- Include telemetry of any kind

## Supported Versions

| Version | Supported |
|---------|-----------|
| 0.2.x   | Yes       |

## Disclosure Policy

For critical vulnerabilities, use a private GitHub issue or reach out via the repository's contact options.
