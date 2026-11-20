---
title: How to use phonedriver with VS Code and GitHub Copilot
seo: How to let GitHub Copilot in VS Code control a phone over MCP
nav: VS Code
description: Add phonedriver to VS Code with one command or .vscode/mcp.json so GitHub Copilot can operate an Android phone or iPhone.
updated: 2026-11-20
date: 2026-11-14
---

VS Code runs standard stdio MCP servers, so GitHub Copilot chat can drive Android and iOS through phonedriver. Run `npx -y phonedriver init` to configure it automatically, or add it directly below.

Note: I develop and benchmark in Claude Code rather than VS Code; the config format below follows [VS Code's MCP reference](https://code.visualstudio.com/docs/copilot/reference/mcp-configuration).

## Add the server

```sh
code --add-mcp '{"name":"phonedriver","command":"npx","args":["-y","phonedriver","mcp"]}'
```

Requires Node 18+. The first run downloads and caches the binary once ([Install](/docs/install/)).

## Or add it to a workspace

Save this as `.vscode/mcp.json` (VS Code uses `servers` as the top-level key instead of `mcpServers`):

```json
{
  "servers": {
    "phonedriver": {
      "type": "stdio",
      "command": "npx",
      "args": ["-y", "phonedriver", "mcp"],
      "env": { "PHONEDRIVER_DEVICE": "android:emulator-5554" }
    }
  }
}
```

To keep the agent in your own app on a phone with real accounts, add `"PHONEDRIVER_APP_SCOPE": "com.example.app"` to `env` ([what it covers](/docs/android-phone/#keep-the-agent-in-your-app)). To let the agent clear, remove or change the permissions of an app you are developing on a physical Android phone (an app Gradle installed, not phonedriver), add `"PHONEDRIVER_APPS_UNDER_TEST": "com.example.app"` to `env`, and run `phonedriver daemon stop` once if a daemon is running.

To keep a phone out of the agent's reach on a shared adb server, add `"PHONEDRIVER_DENY_DEVICES": "android:<id>"` to `env` (or write `deny android:<id>` in `~/.phonedriver/devices`, which every agent on the machine obeys); a denied device is not listed, not probed by `doctor` and never gets the agent.

`PHONEDRIVER_DEVICE` is optional when only one device is connected (`phonedriver devices` lists IDs). On Windows, if VS Code struggles to launch `npx` directly, set `"command": "cmd"` and `"args": ["/c", "npx", "-y", "phonedriver", "mcp"]`.

## Connect a device and ask

Boot an [Android emulator](/docs/android-emulator/), plug in an [Android phone](/docs/android-phone/), or start the [iOS Simulator](/docs/ios-simulator/). Run `npx -y phonedriver doctor` in the terminal to verify everything is `ok`, then ask Copilot chat:

> Open Settings and turn on dark mode.

All 18 tools (`devices`, `apps`, `snapshot`, `screenshot`, `tap`, `type`, `swipe`, `scroll`, `press`, `launch`, `wait`, `batch`, `clipboard`, `shell`, `setting`, `logs`, `network`, `replay`) become available, with `snapshot`, `screenshot`, `wait`, `logs`, and `network` annotated read-only.

## Approval prompts

The [approval guard](/docs/approval-guard/) pauses payment and account-deletion taps via MCP elicitation. If your client version doesn't display the prompt, phonedriver taps nothing and returns `Needs the human's approval; nothing was tapped` so Copilot asks you in chat.

## Questions

### Does phonedriver work with GitHub Copilot?

Yes—Copilot chat uses any stdio MCP server registered in VS Code. If a config step here changes across VS Code updates, [open an issue]({{REPO}}/issues).

### Where does the entry live?

`.vscode/mcp.json` scopes the server to one workspace and can be checked into Git. You can also run VS Code's `MCP: Add Server` command to configure it globally or per workspace.

### Can I use the phonedriver CLI from the VS Code terminal instead?

Yes. `npx -y phonedriver snapshot` and `npx -y phonedriver tap @e4` return the exact same text as the MCP tools.
