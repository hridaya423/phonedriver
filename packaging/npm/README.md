# phonedriver

Let AI agents see and control Android and iOS phones. This package is a launcher: on first run it downloads the `phonedriver` binary for your platform from [GitHub Releases](https://github.com/hridaya423/phonedriver/releases), checks its SHA-256 against the `.sha256` file published beside it, caches it, and runs it. Later runs start the cached binary with no network.

```sh
npx -y phonedriver mcp                                    # serve MCP on stdio
claude mcp add phonedriver -- npx -y phonedriver mcp      # add to Claude Code
npx -y phonedriver --version
```

The launcher contacts GitHub only to download the binary, once per version and platform. It sends no telemetry. The binary is cached under `~/Library/Caches/phonedriver/npm` (macOS), `$XDG_CACHE_HOME` or `~/.cache/phonedriver/npm` (Linux), or `%LOCALAPPDATA%\phonedriver\npm` (Windows).

Platforms: macOS arm64 and x64, Linux x64 and arm64 (glibc), Windows x64. Needs Node 18 or later and `tar` (with xz support on macOS and Linux; Windows 10 and later ships it).

Docs: https://phonedriver.dev. Source and license (AGPL-3.0-only): https://github.com/hridaya423/phonedriver.
