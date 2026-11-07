# GitHub description, topics and social preview (proposed, not applied)

The repository does not exist yet (`api.github.com/repos/hridaya423/phonedriver` returns 404 on 2026-11-07, and so does `hridaya423/phone-use`). Nothing here is applied. Apply it right after the repository is created and before anything is submitted anywhere:

```sh
gh repo create hridaya423/phonedriver --public --source . --description "<description>"
gh repo edit hridaya423/phonedriver --homepage https://phonedriver.dev --add-topic mcp --add-topic mcp-server ...
```

Creating the remote is a one-way door in `AGENTS.md`: the user runs it, or says yes first.

## Canonical sentence

> phonedriver is an open-source MCP server and CLI that lets AI agents like Claude Code see and control Android and iOS phones.

Used verbatim at the top of `README.md`, in `Cargo.toml`, `llms.txt`, `docs/README.md`, and as the first line of every directory entry in `directories.md`.

## Description (106 characters, unchanged)

```text
Open-source MCP server and CLI that lets AI agents like Claude Code see and control Android and iOS phones.
```

## Homepage

`https://phonedriver.dev`. Set it once the site answers with 200. Whois reports `phonedriver.dev` as ACTIVE on 2026-11-07, but `dig` returned no address, so the site is not deployed. [USER: confirm you own the domain and the deploy.]

## Topics (11 of GitHub's 20 allowed)

```text
mcp mcp-server model-context-protocol ai-agents claude-code android ios adb mobile-automation rust cli
```

Each was checked on 2026-11-07 with `gh search repos --topic <t> --limit 1`. Three optional topics to add if you want the full 14: `ui-automation`, `agentic-ai`, `computer-use`. Run the same check on each before adding. `mcp-server` and `claude-code` are the two topics where the people who star MCP servers browse.

## Social preview (needs work before launch)

GitHub has no API for the social preview: upload it under Settings, General, Social preview (1280x640, under 1 MB).

| File | State on 2026-11-07 (opened and looked at) | Use |
|---|---|---|
| `.github/assets/social-preview.png` (1280x640) | Rebuilt with the phonedriver name (D28), looked at 2026-11-07. Numbers are right (41x, 27x, 98x, 19x, 4.6x). | Upload in repo Settings. |
| `.github/assets/hero-social.png` (1200x675) | "Up to 98x faster than adb", four bars, no product name in the image. Numbers match `speed.json`. | Good as the X and LinkedIn card image: the name is in the post. Add a small `phonedriver` wordmark if the build script allows. |
| `.github/assets/tokens-light.png` | Rebuilt 2026-11-07: phonedriver round 3 against phone-harness, per task and sum (218,357 against 330,375), looked at. | Fine for posts. It is three tasks only; the jobs benchmark has its own numbers in `docs/bench/jobs.md`. |

## Other repository settings to do before launch

- Enable Discussions (a place for "what app should it drive" answers that is not an issue).
- Pin an issue titled "Known limits" that links the README limits and `docs/decisions.md` D27. It answers the skeptic questions before they are asked.
- Add `good first issue` labels to three real issues: Reddit and HN readers click Issues first.
- Releases: a tagged release with five binaries (macOS arm64 and x86_64, Linux x86_64 and arm64, Windows x86_64) and the installers. The README says "nothing is published yet" until this exists.
- Add `server.json` and `glama.json` at the repo root (templates in `directories.md`).
- Security policy: `SECURITY.md` with a contact address. The guard and the shell are what a security reader opens first.
