# GitHub description and topics (proposed, not applied)

No remote exists yet, so nothing here is applied. Apply after the repository is created: `gh repo edit --description "<description>" --add-topic <topics>`. The code and repository rename to phonedriver runs later in one pass.

## Canonical sentence

> phonedriver is an open-source MCP server and CLI that lets AI agents like Claude Code see and control Android and iOS phones.

Used verbatim at the top of `README.md`, in `Cargo.toml` (`description`), `llms.txt` and `docs/README.md`.

## Description (106 characters)

```text
Open-source MCP server and CLI that lets AI agents like Claude Code see and control Android and iOS phones.
```

## Topics (11)

```text
mcp mcp-server model-context-protocol ai-agents claude-code android ios adb mobile-automation rust cli
```

Each topic was checked on 2026-11-07 with `gh search repos --topic <t> --limit 1`; every one returns at least one repository (for example `mobile-automation` returns mobile-next/mobile-mcp, `claude-code` returns morluto/rea).

Homepage: leave empty until phonedriver.dev is registered.

Social preview: upload `.github/assets/hero-social.png` (1200x675) or `.github/assets/social-preview.png` (1280x640) under Settings, General, Social preview. GitHub has no API for it.
