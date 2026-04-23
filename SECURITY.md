# Security Policy

## Reporting a Vulnerability

If you discover a security vulnerability in dbscope, please report it responsibly.

**Email:** security@dbscope.dev

Please include:

- Description of the vulnerability
- Steps to reproduce
- Potential impact
- Suggested fix (if any)

We will acknowledge receipt within 48 hours and aim to provide a fix or mitigation within 7 days for critical issues.

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
| 0.1.x   | Yes       |

## Disclosure Policy

We follow coordinated disclosure. Please do not open a public GitHub issue for security vulnerabilities. Use the email above.
