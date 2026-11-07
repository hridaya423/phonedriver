# Directories: where to list phonedriver, and what to paste

Checked 2026-11-07 by fetching each page (a research agent fetched them; sources inline). Nothing is submitted. Every submission is a step for you: some need your GitHub login, some a payment decision, some a one-way door (a public repo, a published crate). Status: **LIVE** (fetched and working), **DEAD** (closed or shut), **UNCLEAR**, **UNVERIFIED** (could not fetch).

Every description reuses the canonical sentence from `github-metadata.md`, trimmed where a field is short.

## Three findings that change the plan

1. **Smithery is DEAD.** Its API, CLI, accounts and proxy stopped on 2026-10-31 (banner on smithery.ai). Do not spend time on it. No `smithery.yaml`.
2. **The official MCP Registry does not take a plain GitHub-release binary.** It lists metadata for a package that already exists elsewhere. Supported types: `npm`, `pypi`, `nuget`, `cargo`, `oci`, `mcpb` (docs: `modelcontextprotocol/registry`, `package-types.mdx`). For a Rust binary that means **`mcpb` (a bundle on GitHub Releases) or `cargo` (crates.io)**. This is a launch blocker for "registry entries live before HN" and needs one decision: see section 1.
3. **Claude's own directory no longer takes local `.mcpb` servers**; a local stdio server goes in as a plugin, with a paid Claude plan and a review (https://claude.com/docs/connectors/building/submission). Not a launch-week item.

## Order, with the day (see `plan.md`)

| Order | Where | When | Effort |
|---|---|---|---|
| 1 | Official MCP Registry | T-2 | 30 min once the package exists |
| 2 | Glama | T-2 | 10 min |
| 3 | punkpeye/awesome-mcp-servers (PR) | T-2, once the repo is public | 15 min |
| 4 | mcpservers.org (free form) | T-2 | 5 min |
| 5 | mcp.so (free issue, if the template exists) | T-1 | 10 min |
| 6 | Own Homebrew tap | with the release | part of release |
| 7 | "Add to Cursor" link on the site and README | T-3 | 10 min |
| 8 | claudemarketplaces.com | T+2 | 10 min |
| 9 | mcpmarket.com (free) | T+2 | 5 min |
| 10 | awesome-claude-code (issue form) | after 14 days or 100 stars | 10 min |
| 11 | Cline, Cursor plugin, Claude plugin directory | T+14 or later | hours |

## 1. Official MCP Registry (LIVE, preview)

URL: https://registry.modelcontextprotocol.io. Method: the `mcp-publisher` CLI only (v1.8.8 on 2026-11-06): `mcp-publisher init`, `mcp-publisher login github` (device code), `mcp-publisher publish`. No web form, no PR, no stated review queue. The registry calls itself preview: data can reset.

Namespace: **`io.github.hridaya423/phonedriver`**, authenticated by GitHub login. It works on day one and needs no DNS. The alternative, `dev.phonedriver/...`, needs a TXT record on `phonedriver.dev` (`v=MCPv1; k=ed25519; p=<base64 key>`, Ed25519 needs OpenSSL 3, macOS LibreSSL fails) or a file at `https://phonedriver.dev/.well-known/mcp-registry-auth`. Do it later if you want the domain namespace.

**Decision needed (blocker):** pick the package route.

| Route | What it needs | Problem |
|---|---|---|
| `mcpb` | A `.mcpb` bundle on a GitHub Release (URL must contain "mcp"), `fileSha256` in `server.json`, a `manifest.json` with `server.type: "binary"`. | One bundle must carry or select a binary per OS and CPU. Whether `manifest.json` can do that for five targets is [UNVERIFIED]: read `modelcontextprotocol/mcpb` `MANIFEST.md` before building. |
| `cargo` | The crate published on crates.io, with the visible line `mcp-name: io.github.hridaya423/phonedriver` in its README (crates.io strips HTML comments). | The README says the build needs the Android SDK, `d8` and JDK 17 unless `PHONE_USE_AGENT_JAR` is set, so `cargo install phonedriver` fails for almost everyone unless the crate ships the built jar. [UNVERIFIED: check what `cargo package` would include.] |
| `npm` wrapper | A thin npm package that downloads or bundles the binary, with `mcpName` in `package.json`. | Extra package to maintain. It does give `npx -y phonedriver mcp`, which is the install line MCP users know. |

Recommendation: **the `mcpb` bundle if the manifest supports per-platform binaries; otherwise an npm wrapper** (a thin package, one binary package per OS as optional dependencies, the pattern esbuild uses). Do not publish a crate that does not build. [USER or AGENT: engineering task, 1 to 2 days.]

`server.json` for the `mcpb` route (schema 2025-12-11 from the registry docs; `description` is limited to 100 characters):

```json
{
  "$schema": "https://static.modelcontextprotocol.io/schemas/2025-12-11/server.schema.json",
  "name": "io.github.hridaya423/phonedriver",
  "title": "phonedriver",
  "description": "Let AI agents see and control Android and iOS phones over stdio.",
  "repository": { "url": "https://github.com/hridaya423/phonedriver", "source": "github" },
  "websiteUrl": "https://phonedriver.dev",
  "version": "0.1.0",
  "packages": [
    {
      "registryType": "mcpb",
      "identifier": "https://github.com/hridaya423/phonedriver/releases/download/v0.1.0/phonedriver.mcpb",
      "fileSha256": "<sha256 of the bundle>",
      "transport": { "type": "stdio" }
    }
  ]
}
```

(The description is 64 characters.) For the `cargo` route replace the package with `{"registryType":"cargo","identifier":"phonedriver","version":"0.1.0","transport":{"type":"stdio"}}`. Whether `packageArguments` must say that the server is the `mcp` subcommand is [UNVERIFIED]: run `mcp-publisher validate` and read the error. Verify after publishing: `curl 'https://registry.modelcontextprotocol.io/v0.1/servers?search=phonedriver'`.

Downstream: PulseMCP picks registry entries up automatically, and Zed points MCP users at the registry.

## 2. Glama (LIVE)

URL: https://glama.ai/mcp/servers, "Add Server", sign in with GitHub, paste the repo URL. The exact form was not shown to the fetch [UNVERIFIED]. Glama also syncs with punkpeye's list, so the PR in section 3 can put you there too.

To claim ownership, add `glama.json` at the repo root, then rerun "Claim ownership" in Glama:

```json
{
  "$schema": "https://glama.ai/mcp/schemas/server.json",
  "maintainers": ["hridaya423"]
}
```

Glama may want a `Dockerfile` for its tool-introspection check [UNVERIFIED]. Test what the page asks for; do not build a Dockerfile before it asks. Paste for description: "phonedriver is an open-source MCP server and CLI that lets AI agents like Claude Code see and control Android and iOS phones."

## 3. punkpeye/awesome-mcp-servers (LIVE, 97k stars)

Method: a pull request that edits `README.md` (https://github.com/punkpeye/awesome-mcp-servers, `CONTRIBUTING.md`). Rules: a public GitHub repo you install and run yourself; alphabetical inside the category; one server per line; one-sentence description, no marketing language; follow the neighbours' format. Add a line to your PR text that you maintain the server (community norm, secondary source). Do **not** add the `🤖🤖🤖` suffix to the PR title: it is for automated agents.

**Category:** `### 💻 Developer Tools`. There is no mobile category; `mobile-next/mobile-mcp`, `iksnerd/adb-mcp`, `hyperb1iss/droidmind` and the iOS Simulator servers all sit there. Position: roughly alphabetical by `owner/repo`; `hridaya423/phonedriver` goes after the `h...` entries such as `hoainho` and before `hyperb1iss/droidmind`. Check the neighbours at PR time.

Legend (exact): `🦀` Rust; `🏠` local; `🍎` macOS, `🪟` Windows, `🐧` Linux. Neighbour lines, exact:

```
- [mobile-next/mobile-mcp](https://github.com/mobile-next/mobile-mcp) 📇 🏠 🐧 🍎 - Android and iOS app and device automation, development and app scraping on simulators, emulators and physical devices.
- [iksnerd/adb-mcp](https://github.com/iksnerd/adb-mcp) [![iksnerd/adb-mcp MCP server](https://glama.ai/mcp/servers/iksnerd/adb-mcp/badges/score.svg)](https://glama.ai/mcp/servers/iksnerd/adb-mcp) 🏎️ 🏠 🍎 🪟 🐧 - Drive Android emulators and devices over adb: screenshots, UI inspection, tap/swipe/type, app lifecycle, logcat, screen recording and Gradle builds and tests.
```

**The line to paste:**

```
- [hridaya423/phonedriver](https://github.com/hridaya423/phonedriver) 🦀 🏠 🍎 🪟 🐧 - See and control Android and iOS phones from AI agents: a numbered text view of the screen, and tap, type and swipe that return only what changed, with a human approval prompt before Pay or Place order.
```

OS emojis: Android runs on macOS, Linux and Windows per the README and `dist-workspace.toml`; the Linux binary was not run in a benchmark (ledger, "Verify"). Keep `🐧` only if you ran it. iOS needs macOS.

If Glama has indexed the repo by then, add the badge after the repo link (optional; some neighbours have none): `[![hridaya423/phonedriver MCP server](https://glama.ai/mcp/servers/hridaya423/phonedriver/badges/score.svg)](https://glama.ai/mcp/servers/hridaya423/phonedriver)`.

**PR title:** `Add hridaya423/phonedriver`
**PR body:**

```
I maintain this server. phonedriver is an open-source (AGPL-3.0) MCP server and CLI that lets AI agents like Claude Code see and control Android and iOS phones. Local stdio, Rust. Category: Developer Tools, next to mobile-next/mobile-mcp and the other mobile servers.
```

## 4. mcpservers.org (the wong2 list) (LIVE)

URL: https://mcpservers.org/submit. The wong2 repo accepts no PRs ("We do not accept PRs. Please submit your MCP on the website"). Free plan: reviewed within 2 weeks. Premium ($39): 24 hours, a badge and a dofollow link. Skip premium: nothing here needs it.

| Field | Paste |
|---|---|
| Server Name | `phonedriver` |
| Category | Development |
| Short Description | `See and control Android and iOS phones from AI agents. Local stdio MCP server and CLI, written in Rust.` |
| Link | `https://github.com/hridaya423/phonedriver` |
| Official MCP Registry Name (optional) | `io.github.hridaya423/phonedriver` (only after section 1 is live) |
| Remote connections | unchecked (it is local) |
| Contact Email | [USER] |
| Plan | Free |

## 5. mcp.so (LIVE)

URL: https://mcp.so/submit (tabs: MCP Server, Remote Server, MCP Client, AI Product; fields Repository URL and Name). The page offers a $39 "publish without review" option. The free route is UNCLEAR: a community answer says to open an issue on `chatmcp/mcpso` titled `Submit: phonedriver`; the template could not be read without sign-in [UNVERIFIED]. Try the form's free path or the issue; do not pay.

Paste: Name `phonedriver`; Repository `https://github.com/hridaya423/phonedriver`.

## 6. PulseMCP (LIVE, submissions PAUSED)

https://www.pulsemcp.com/submit says new submissions are paused with no date, and that servers in the official registry are ingested automatically. Action: do section 1 and nothing else. A secondary source says to email hello@pulsemcp.com after about a week [UNVERIFIED]; do not.

## 7. awesome-claude-code lists

**hesreallyhim/awesome-claude-code** (LIVE, 57k stars). Issue form **only**, by a human in the web UI (not through `gh`): https://github.com/hesreallyhim/awesome-claude-code/issues/new?template=recommend-resource.yml. A bot validates the form. Rules: the repo must be **at least 14 days old with ongoing commits, or have 100+ stars**; one resource at a time; a wrong submission risks a temporary restriction; one-line description, no emoji, no sales pitch; the resource must be specific to Claude Code. So this waits until T+14 or 100 stars.

| Field | Paste |
|---|---|
| Display Name | `phonedriver` |
| Category | Providers, Runtime & Integration Infrastructure (alternative: Testing) |
| Link | `https://github.com/hridaya423/phonedriver` |
| Author Name / Link | `Hridya Agrawal` / `https://github.com/hridaya423` |
| Description | `An MCP server and CLI that lets Claude Code see and control Android and iOS phones, with a numbered text view of the screen, a diff after every action, and an approval prompt before it taps Pay or Place order.` |

Checklist boxes: tick the true ones; the last one is a trap box and must stay unchecked. Existing entry format (a bot adds badges): `- [Name](repo) by [Author](profile) - Description.`

Other lists: `jqueryscript/awesome-claude-code` (live, method UNVERIFIED), `rohitg00/awesome-claude-code-toolkit` (probably stale). Skip both for launch week.

## 8. Claude Code plugin and skill marketplaces

- **Anthropic's plugin directory** (`anthropics/claude-plugins-official`, live): submit through the developer portal https://claude.ai/directory/manage, Submit new, Plugin bundle, then Validate, listing details, data-handling answers, four compliance acknowledgements, Submit for review (https://claude.com/docs/plugins/submit.md). Needs any paid Claude plan, a public repo at go-live, a `.claude-plugin/plugin.json`, a README, a LICENSE and an MCP config; every version gets a security scan; the review time is not stated. A stdio server that needs a binary on PATH needs the install story settled first. **T+14 or later.** [USER: the portal needs your account.]
- **Claude Desktop extensions (`.mcpb`)**: the directory **no longer accepts local servers packaged as MCPB**. A `.mcpb` file can still be shipped on the Releases page for manual install; it also feeds the registry route in section 1.
- **claudemarketplaces.com** (live, independent): https://claudemarketplaces.com/submit, sign in with GitHub, submit the repo as an MCP server. Free; priority review costs $4 (skip). T+2.
- **buildwithclaude.com**: PR-based and plugins and skills only, not MCP servers. Skip unless you build a plugin.
- **skillsmp.com**: 403, UNVERIFIED.

## 9. Cursor, Cline, other clients

- **"Add to Cursor" deeplink** (documented: https://cursor.com/docs/context/mcp/install-links). Not a directory; put the button on the site and README:

  ```
  cursor://anysphere.cursor-deeplink/mcp/install?name=phonedriver&config=eyJjb21tYW5kIjoicGhvbmVkcml2ZXIiLCJhcmdzIjpbIm1jcCJdfQ==
  ```

  The config is base64 of `{"command":"phonedriver","args":["mcp"]}`. The site's Cursor page says the Cursor path is untested, so test the link in Cursor before publishing it. [USER]
- **cursor.directory**: takes plugins now, not bare MCP servers. https://cursor.directory/plugins/new (the fetch of the submit page hit a 429; the path comes from the README of `pontusab/directories`) with GitHub or Google sign-in and a repo URL; components are detected from `.mcp.json`, `skills/*/SKILL.md` and similar; an automated security scan runs. T+14, with a tested Cursor setup.
- **Cline MCP marketplace** (`cline/mcp-marketplace`, low activity: last push 2025-06-24): open an issue from https://github.com/cline/mcp-marketplace/issues/new?template=mcp-server-submission.yml with the repo URL and a **400x400 PNG logo**, and tick that Cline can set the server up from the README alone and that the server is stable. They vet for community adoption. T+14 or later; make the 400x400 PNG when you rebuild brand assets.
- **Zed**: points MCP users at the official registry. Covered by section 1.
- **Windsurf, Continue hub**: no submission path found [UNVERIFIED]. Skip.
- **VS Code MCP gallery and github.com/mcp**: backed by the GitHub MCP Registry; self-publishing is not documented. Do section 1 first, then look for GitHub's onboarding discussion [UNVERIFIED].

## 10. Awesome lists for Android and iOS tooling

| List | Verdict |
|---|---|
| vsouza/awesome-ios (54k stars) | **Do not submit.** Hard rules: 100+ stars, Swift Package Manager support, Swift 5.5+ or Objective-C. A Rust CLI fails them; the PR would be closed. |
| JStumpp/awesome-android | Live, stale (last push 2025-10-27). Low value; optional later PR under "Testing and Debugging Tools" in its `- [Name](url) - description` format. |
| SrinivasanTarget/awesome-appium (464 stars, live) | A stretch: Appium list. Format ``[name](link) - Description.`` alphabetical, capital letter, trailing period. Skip. |
| atinfo/awesome-test-automation (stale since 2025-11) | Optional PR to `mobile-test-automation.md`, "Cross-platform tools": `* [phonedriver](https://github.com/hridaya423/phonedriver) - Lets AI agents see and control Android and iOS phones through MCP and a CLI.` Weak fit. |
| e2b-dev/awesome-ai-agents | Lists agent products. Weak fit. |
| ranpox/awesome-computer-use | Papers and benchmarks; takes tools [UNVERIFIED]. Skip for now. |
| tolkonepiu/best-of-mcp-servers (410 projects, auto-ranked) | Open an issue from its template later. Ranking is by stars, so low value before launch. |
| TensorBlock/awesome-mcp-servers | Live, process UNVERIFIED. Skip. |
| awesome-android-testing, awesome-mobile-testing, JamesTrue/awesome-android | Not found or no traction. Dropped. |

## 11. Other sites

| Site | Status | Action |
|---|---|---|
| mcpmarket.com | LIVE | https://mcpmarket.com/submit: repo URL and email. Free takes about 4 to 6 weeks; $29 is 24 hours. Free only. T+2. |
| MCP Server Finder | LIVE per a community reply | Email info@mcpserver.com: name, one-line description, repo URL, category tags [secondary source]. Optional. |
| opentools.com | UNCLEAR | Now an agentic-commerce product with no submit flow; /submit returns 404. Dropped. |
| mcp-get.com, mcphub.io | UNVERIFIED | Dropped. |
| alternativeTo | UNVERIFIED | Submit pages returned 403 and 404, and the category fit is weak. Dropped; try by hand later. |
| GitHub MCP Registry (github.com/mcp) | UNCLEAR | See section 9. |
| Docker MCP catalog (`docker/mcp-registry`) | LIVE, **poor fit** | Dropped. It wants a Dockerfile and its guidelines say "MIT or Apache 2 are great, GPL is not" (AGPL likely rejected). A container also isolates the host `adb`, USB and the macOS stack phonedriver needs. |
| Smithery | **DEAD** | Dropped. |
| appcypher/awesome-mcp-servers | **DEAD** (archived 2026-05-06) | Dropped. |
| modelcontextprotocol/servers | LIVE | Takes no third-party entries (reference servers only). Dropped. |
| punkpeye/awesome-remote-mcp-servers | LIVE | Remote only. Not applicable. |

## 12. Packages and install paths

- **Own Homebrew tap**: `dist-workspace.toml` already sets `tap = "hridaya423/homebrew-tap"` and the `homebrew` installer. The README line is `brew install hridaya423/tap/phonedriver` after the rename. Per Homebrew docs a non-official tap may need explicit trust (`brew trust --tap hridaya423/tap`) [secondary reading of the docs; test on a clean Mac and write what you see in the README].
- **homebrew-core**: not eligible. A self-submission needs 225 stars, 90 forks or 90 watchers, and the repo should be 30 days old (https://docs.brew.sh/Package-Acceptance-Policy). Revisit after launch.
- **crates.io**: only if the crate builds without the Android SDK (see section 1).
- **npm wrapper**: see section 1. `phone-use` is taken on npm; `phonedriver` returned 404 on npm, crates.io and PyPI on 2026-11-07, so the name is free today. Reserve it by publishing the real package, not a placeholder (a placeholder is squatting and reads badly).

## 13. Files to add to the repo

| File or asset | Needed by |
|---|---|
| `server.json` | official registry |
| A visible line `mcp-name: io.github.hridaya423/phonedriver` in the README (needed for `cargo`; harmless otherwise) | official registry |
| The published package (`.mcpb`, npm or crate) | official registry |
| `glama.json` | Glama claim |
| 400x400 PNG logo | Cline |
| `.claude-plugin/plugin.json`, `.mcp.json` | Claude plugin directory, cursor.directory (T+14) |
| `SECURITY.md` and a "Privacy" section in the README (no telemetry, D9) | Claude directory and `.mcpb` rules |
| The "Add to Cursor" button | site and README |

## What the entries say, in one place

Always the same facts, nothing a ledger row does not back: open-source (AGPL-3.0), MCP server and CLI, Android and iOS, local stdio, no telemetry, approval prompt before payments. Do not put "98x" in a directory entry (it needs its qualifier); put it in the README those entries link to.
