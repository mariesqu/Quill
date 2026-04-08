---
name: Bug report
about: Something isn't working as expected
title: "[Bug] "
labels: bug
assignees: ""
---

## Describe the bug

A clear and concise description of what the bug is.

## Steps to reproduce

1. Select text in '...'
2. Press hotkey
3. Click mode '...'
4. See error

## Expected behaviour

What you expected to happen.

## Actual behaviour

What actually happened. Include any error messages shown in the overlay.

## Environment

| Field | Value |
|---|---|
| OS | e.g. macOS 14.4 / Windows 11 / Ubuntu 24.04 |
| Quill version | e.g. 0.2.0 |
| Install type | release installer / built from source |
| Provider | e.g. OpenRouter / Ollama / OpenAI |
| Model | e.g. google/gemma-3-27b-it |

## Logs

Run Quill from a terminal with tracing enabled and paste any relevant output:

```bash
# From a release install
RUST_LOG=quill=debug quill

# Or from source
cd ui && RUST_LOG=quill=debug npm run tauri dev
```

```
paste logs here
```

## Additional context

Any other context, screenshots, or screen recordings.
