---
title: How to use phonedriver with Cursor and other MCP clients
nav: Cursor and other clients
description: Add phonedriver to Cursor or any MCP client that starts a stdio server, with a JSON entry, an optional device setting and the list of tools it exposes.
---

Any client that starts a stdio MCP server can use phonedriver. We test with Claude Code. We have not run a Cursor session ourselves, so the Cursor steps below use the standard stdio entry and Cursor's documented configuration file, not something we watched work.

## The entry

```json
{
  "mcpServers": {
    "phonedriver": {
      "command": "phonedriver",
      "args": ["mcp"],
      "env": { "PHONEDRIVER_DEVICE": "android:emulator-5554" }
    }
  }
}
```

If `phonedriver` is not on the `PATH` the client sees, put the full path in `command`. `PHONEDRIVER_DEVICE` is optional when one device is connected.

## Cursor

Put the entry in `~/.cursor/mcp.json` for every project, or in `.cursor/mcp.json` inside one project. Open Cursor's MCP settings and check that `phonedriver` shows its tools.

## Other clients

Use the client's own place for stdio servers, with the same `command` and `args`. If the client cannot start a local process, run `phonedriver` where the client can reach it and forward the daemon's socket (see [Android emulator](/docs/android-emulator/#use-a-device-on-another-machine)).

## Tools

The server has fourteen tools: `devices`, `apps`, `snapshot`, `screenshot`, `tap`, `type`, `swipe`, `scroll`, `press`, `launch`, `wait`, `batch`, `clipboard` and `shell`. `apps` also installs, stops, clears and uninstalls. Each prints the same text as the CLI verb of the same name, so anything you read in these docs about the command line holds for the tools.

In Claude Code's wrapper the fourteen tools cost about 51 tokens each whatever their text. The first turn of a one-word `claude -p` costs 5,606 tokens with phonedriver attached ([token benchmark](/benchmarks/#tokens)).

## Approval prompts need elicitation

The [approval guard](/docs/approval-guard/) asks you through MCP elicitation. A client that does not support elicitation cannot show the question, and the tool then replies that the tap needs the human's approval and taps nothing. Check that your client supports elicitation before you give an agent a phone with a shopping app on it.
