# Security Policy

## Supported Versions

| Version | Supported |
|---|---|
| 0.1.x (latest) | Yes |

---

## Reporting a Vulnerability

**Please do not open a public GitHub issue for security vulnerabilities.**

If you discover a security issue, report it privately:

1. Go to the [Security tab](https://github.com/mariesqu/Quill/security/advisories/new) of this repository
2. Click **"Report a vulnerability"**
3. Fill in the details — what you found, how to reproduce it, and potential impact

You will receive a response within **48 hours** acknowledging the report. We aim to release a fix within **14 days** for critical issues.

---

## Scope

Issues we consider in-scope:

- Arbitrary code execution via crafted config files or IPC messages
- API key leakage (written to logs, transmitted unexpectedly, exposed in UI)
- Local privilege escalation
- Clipboard data exfiltration beyond the user's selected text
- Injection attacks through AI provider responses rendered in the UI

Out of scope:

- Vulnerabilities in third-party AI providers (report to them directly)
- Issues requiring physical access to the user's machine
- Social engineering attacks

---

## Security Design Notes

Quill is designed with privacy as a first principle:

- **No telemetry** — no analytics, no crash reporting, no usage tracking
- **API keys** stored only in `config/user.yaml` (gitignored) or environment variables — never transmitted anywhere except your chosen AI provider
- **History** stored locally in `~/.quill/history.db` — opt-in, never synced
- **IPC** is stdin/stdout between Tauri and the Python sidecar — no network socket
- **Text you transform** is sent only to the AI provider you explicitly configured

---

## Disclosure Policy

We follow [responsible disclosure](https://en.wikipedia.org/wiki/Coordinated_vulnerability_disclosure). Once a fix is released, we will credit the reporter in the release notes unless they prefer to remain anonymous.
