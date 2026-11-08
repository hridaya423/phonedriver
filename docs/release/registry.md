# Official MCP Registry and MCPB: what was checked, and what phonedriver does

Checked 2026-11-07 by fetching the sources below. `mcp-publisher` v1.8.8 was downloaded and run (`validate` only). Nothing was published.

## Registry requirements

Sources: `modelcontextprotocol/registry` repo, `docs/modelcontextprotocol-io/quickstart.mdx`, `package-types.mdx`, `docs/reference/server-json/generic-server-json.md`; the schema at https://static.modelcontextprotocol.io/schemas/2025-12-11/server.schema.json (JSON Schema draft-07); releases at https://github.com/modelcontextprotocol/registry/releases.

- The registry hosts metadata only. The package must already exist on a supported registry. Supported types: `npm`, `pypi`, `nuget`, `cargo`, `oci`, `mcpb`. A plain GitHub-release binary is not accepted.
- **npm:** only the public npm registry. `package.json` must carry `mcpName`, equal to the `name` in `server.json`. Without it publish fails with "Registry validation failed for package".
- **mcpb:** the artifact must be on a GitHub or GitLab release; the URL (`identifier`) must contain the string "mcp"; `server.json` must carry `fileSha256` (`openssl dgst -sha256 file.mcpb`). The registry does not check the hash; clients do.
- **Names:** with GitHub login the name must start with `io.github.<github-username>/`. Ours is `io.github.hridaya423/phonedriver`.
- `description` is limited to 100 characters (ours is 64).
- **Flow:** install `mcp-publisher` (`brew install mcp-publisher`, or the release tarball), `mcp-publisher login github` (device code), `mcp-publisher publish`. Verify with `curl "https://registry.modelcontextprotocol.io/v0.1/servers?search=io.github.hridaya423/phonedriver"`.
- The registry is in preview; data can be reset.

## MCPB spec

Source: `modelcontextprotocol/mcpb`, `MANIFEST.md`.

- Server type `binary` supports "platform-specific binaries". `platform_overrides` in `mcp_config` can change `command`, `args` and `env` per OS. `compatibility.platforms` takes `darwin`, `win32`, `linux`. `${__dirname}` is the bundle directory.
- There is no CPU architecture field anywhere. A single bundle cannot select between arm64 and x64 on the same OS.

## Decision (D29): npm launcher, no MCPB bundle

The npm launcher is the shortest install: `claude mcp add phonedriver -- npx -y phonedriver mcp` is one command, needs no prior install, and works in every client that takes a command. The registry's `npm` type points straight at it, and `npx` is the line MCP users already know.

An `.mcpb` bundle is not added:

- It cannot choose by CPU. macOS arm64 and x64 would each need their own bundle, and the registry entry would list several packages for one server. A fat bundle with both binaries (and Linux and Windows) would be about 17 MB of binaries per download. (Archive sizes: the aarch64 macOS `.tar.gz` is 3.3 MB; the other four are not built yet.)
- The unsigned binary inside a bundle would meet the user's client in the way the client decides, and we cannot test Claude Desktop installs here.
- The bundle would need a second release asset, a hash in `server.json` that changes with every build, and a version-bump step per release. The npm route has none of that.
- Claude's own directory no longer takes local `.mcpb` servers.

Revisit if Claude Desktop users ask for double-click install. Then ship one `node`-type bundle that runs the same launcher (no architecture problem, because the launcher downloads the right binary).

## What phonedriver ships

| File | Role |
|---|---|
| `packaging/npm/package.json` | package `phonedriver`, `mcpName: io.github.hridaya423/phonedriver`, bin `phonedriver` |
| `packaging/npm/bin/phonedriver.js`, `lib/launcher.js` | the launcher |
| `packaging/npm/test/launcher.test.js` | offline tests with a fake release directory |
| `server.json` | registry metadata, npm package, `runtimeHint: npx`, positional argument `mcp` |

## How the launcher works

1. Maps `process.platform` and `process.arch` to a cargo-dist target: `aarch64-apple-darwin`, `x86_64-apple-darwin`, `x86_64-unknown-linux-gnu`, `aarch64-unknown-linux-gnu`, `x86_64-pc-windows-msvc`. Anything else exits with a message and a link to the other installs.
2. If `<cache>/<version>/<target>/phonedriver[.exe]` exists, runs it. No network.
3. Otherwise downloads `phonedriver-<target>.tar.gz` (`.zip` on Windows) and `.sha256` from `https://github.com/hridaya423/phonedriver/releases/download/v<version>/`, checks the SHA-256, unpacks with the system `tar`, moves the binary into the cache and runs it. Its progress line goes to stderr, because stdout is the MCP stream. The package version is the release version, so the launcher and binary always match.
4. Passes arguments, stdin, stdout, signals and the exit code through.

No postinstall script: nothing runs at `npm install` time, and the download waits for the first real use. The only outbound request is the download from GitHub Releases (D9 holds: no telemetry, no other host). The checksum is fetched from the same release, so it catches corruption and truncated downloads, not a compromised release; that is the same trust the shell installer has.

Test overrides: `PHONEDRIVER_RELEASE_BASE_URL` (an `https:` or `file:` URL of a directory holding the archive and its `.sha256`) and `PHONEDRIVER_NPM_CACHE`.

## macOS Gatekeeper and quarantine

The launcher fetches with Node, which does not set the `com.apple.quarantine` attribute (only browsers and some apps do), so macOS does not show a Gatekeeper prompt for the cached binary. The binary is linker-signed (ad hoc) by the Rust toolchain on arm64, which is what Apple Silicon requires to run (`codesign -dv` on the cached file shows a Mach-O arm64 signature identifier). cargo-dist does not notarize or Developer-ID sign anything. Browser-downloaded archives from the Releases page would be quarantined; users who do that run `xattr -d com.apple.quarantine phonedriver`. Windows SmartScreen acts on the Mark of the Web, which the launcher does not set either. Not tested on a clean Mac, a Linux box or Windows: only macOS arm64 ran here.

## Known limits

- Unix archives are `.tar.gz` (`unix-archive = ".tar.gz"` in `dist-workspace.toml`), so the launcher needs only a `tar` with gzip, which every macOS, Linux and Windows 10+ system has. This was switched from cargo-dist's default `.tar.xz` because minimal Linux containers often lack `xz`.
- Linux is glibc only (the cargo-dist target). Alpine (musl) is not supported.
- The Windows `.zip` path and the Linux archives were not run: only the macOS arm64 archive was built and served.
