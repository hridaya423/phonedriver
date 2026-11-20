---
title: How to use phonedriver with Cursor and other MCP clients
seo: How to let Cursor, Gemini CLI, Zed or Cline control a phone
nav: Cursor and other clients
description: Add phonedriver to Cursor, Gemini CLI, Zed, Cline, or any stdio MCP client, with the exact config snippet for each.
updated: 2026-11-20
---

Any client that can launch a stdio MCP server can drive phones with phonedriver: command `npx`, args `["-y", "phonedriver", "mcp"]`. `npx -y phonedriver init` configures Cursor and Gemini CLI automatically; [Codex](/docs/codex/) and [VS Code](/docs/vs-code/) have dedicated guides.

Note: Claude Code is my daily driver, so the snippets for Cursor, Gemini CLI, Zed, and Cline below come from their official MCP docs.

## The standard snippet

```json
{
  "mcpServers": {
    "phonedriver": {
      "command": "npx",
      "args": ["-y", "phonedriver", "mcp"],
      "env": { "PHONEDRIVER_DEVICE": "android:emulator-5554" }
    }
  }
}
```

To keep the agent in your own app on a phone with real accounts, add `"PHONEDRIVER_APP_SCOPE": "com.example.app"` to `env` ([what it covers](/docs/android-phone/#keep-the-agent-in-your-app)). To let the agent clear, remove or change the permissions of an app you are developing on a physical Android phone (an app Gradle installed, not phonedriver), add `"PHONEDRIVER_APPS_UNDER_TEST": "com.example.app,com.example.other"` to `env`, and run `phonedriver daemon stop` once if a daemon is running.

To keep a phone out of the agent's reach on a shared adb server, add `"PHONEDRIVER_DENY_DEVICES": "android:<id>"` to `env` (or write `deny android:<id>` in `~/.phonedriver/devices`, which every agent on the machine obeys); a denied device is not listed, not probed by `doctor` and never gets the agent.

The first start downloads and caches the binary once. Drop `env` if only one device is connected; if you installed the binary permanently, use `"command": "phonedriver"` and `"args": ["mcp"]`. On Windows, some clients need `"command": "cmd"` with `"args": ["/c", "npx", "-y", "phonedriver", "mcp"]`.

## Cursor

Drop the snippet into `~/.cursor/mcp.json` (global) or `.cursor/mcp.json` (per project), then confirm `phonedriver` appears in Cursor's MCP settings. [Cursor's MCP docs](https://cursor.com/docs/context/mcp) list elicitation support, which powers the payment approval prompt.

## Gemini CLI

Add the snippet to `~/.gemini/settings.json` (global) or `.gemini/settings.json` (project). Run `gemini mcp list` or `/mcp` inside a session to verify ([Gemini CLI docs](https://geminicli.com/docs/tools/mcp-server/)).

## Zed

Zed calls MCP servers `context_servers` in its settings file (`zed: open settings file`). Per [Zed's MCP docs](https://zed.dev/docs/ai/mcp), elicitation isn't supported yet, so held payment taps will return a message telling the agent to ask you in chat.

```json
{
  "context_servers": {
    "phonedriver": {
      "command": "npx",
      "args": ["-y", "phonedriver", "mcp"],
      "env": {}
    }
  }
}
```

## Cline

Open the MCP Servers panel → **Configure** → **Configure MCP Servers** to open `cline_mcp_settings.json`, and paste the standard snippet ([Cline docs](https://docs.cline.bot/mcp/configuring-mcp-servers)).

## Remote or custom clients

Point any other stdio MCP client at `npx -y phonedriver mcp`. If your client can't spawn local processes near the phone, run phonedriver on the host that has the device and forward the daemon socket over SSH ([remote device guide](/docs/android-emulator/#use-a-device-on-another-machine)).

## Tools & token footprint

All 18 tools (`devices`, `apps`, `snapshot`, `screenshot`, `tap`, `type`, `swipe`, `scroll`, `press`, `launch`, `wait`, `batch`, `clipboard`, `shell`, `setting`, `logs`, `network`, `replay`) mirror the CLI's text output word for word; `apps` also installs, stops, clears and uninstalls. In Claude Code's wrapper a tool costs about 51 tokens whatever its text. Measured when the server had 14 tools, they added ~5,606 tokens on turn one ([token benchmark](/benchmarks/#tokens)).

## Approval prompts need elicitation

The [approval guard](/docs/approval-guard/) asks you directly through MCP elicitation. In clients without elicitation support, phonedriver refuses the tap safely and tells the agent to ask you in chat.

## Questions

### Which MCP clients does phonedriver work with?

Any client that speaks MCP over stdio—including Claude Code, Codex, Cursor, VS Code, Gemini CLI, Zed, and Cline—plus any shell-capable agent via the CLI.

### What is the MCP config for phonedriver?

A stdio server running `npx -y phonedriver mcp`. Most clients nest it under `mcpServers` in JSON; VS Code uses `servers`, Zed uses `context_servers`, and Codex uses `[mcp_servers.phonedriver]` in `config.toml`.

### Does the approval prompt work in every client?

Only in clients with MCP elicitation (such as Claude Code and Cursor). In clients without it, phonedriver still holds the tap, touches nothing, and tells the agent to ask you in chat.
