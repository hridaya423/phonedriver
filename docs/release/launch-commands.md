# Launch-day commands, in order

Everything here is yours to run; nothing has been run. Run them from the repo root unless a step says otherwise. Each step ends with a check. Do not go on if the check fails. Background: `registry.md` (why npm), `../release-checklist.md` (decisions and CI risks), `../launch/directories.md` (directory text).

Order matters: the launcher downloads from the GitHub Release, so the release must exist before `npm publish`; the registry checks the npm package, so `npm publish` must come before `mcp-publisher publish`.

## 0. Before anything

```sh
node --test packaging/npm/test/*.test.js     # 5 pass
python3 site/check.py                         # problems 0
mcp-publisher --help || brew install mcp-publisher
gh auth status                                # logged in as hridaya423
npm whoami || npm login                       # needs an npm account with 2FA
```

Also from `release-checklist.md` section 0: the Homebrew tap repo `hridaya423/homebrew-tap` and the `HOMEBREW_TAP_TOKEN` secret (or drop `homebrew` from `dist-workspace.toml`), and the CI proof on a pull request. The versions in `Cargo.toml`, `packaging/npm/package.json` and `server.json` must agree (a test checks it).

## 1. Create the repo and push (one-way door)

```sh
gh repo create hridaya423/phonedriver --public --source=. --remote=origin --push
```

Verify: `gh repo view hridaya423/phonedriver --json visibility,url` shows `PUBLIC`; the Actions tab shows `ci` running.

## 2. Tag the release (cargo-dist builds the binaries)

Wait for CI to be green on `main`, then:

```sh
git tag v0.1.0
git push origin v0.1.0
gh run watch                                  # the Release workflow
```

Verify (when the run is green):

```sh
gh release view v0.1.0 --json assets --jq '.assets[].name'
```

The list must include `phonedriver-aarch64-apple-darwin.tar.gz`, `phonedriver-x86_64-apple-darwin.tar.gz`, `phonedriver-x86_64-unknown-linux-gnu.tar.gz`, `phonedriver-aarch64-unknown-linux-gnu.tar.gz`, `phonedriver-x86_64-pc-windows-msvc.zip`, each with a `.sha256`, plus the two installer scripts. Then:

```sh
curl --proto '=https' --tlsv1.2 -LsSf https://github.com/hridaya423/phonedriver/releases/latest/download/phonedriver-installer.sh | sh
phonedriver version                           # phonedriver 0.1.0
```

If a target failed to build, fix and retag before going on; the launcher needs all five archives for its users.

## 3. Publish the launcher to npm

```sh
cd packaging/npm
npm pack --dry-run                            # lists bin/, lib/, README.md, LICENSE, package.json
npm publish --access public                   # asks for your 2FA code
cd ../..
```

Verify:

```sh
npm view phonedriver version mcpName          # 0.1.0 and io.github.hridaya423/phonedriver
cd "$(mktemp -d)" && npx -y phonedriver --version    # downloads once, prints: phonedriver 0.1.0
npx -y phonedriver --version                  # second run: no download message
```

Then on a clean machine: `claude mcp add phonedriver -- npx -y phonedriver mcp`, `claude mcp list` shows Connected.

## 4. Publish to the official MCP Registry

```sh
mcp-publisher validate                        # server.json is valid
mcp-publisher login github                    # open the URL, enter the code, as hridaya423
mcp-publisher publish
```

Verify:

```sh
curl -s "https://registry.modelcontextprotocol.io/v0.1/servers?search=io.github.hridaya423/phonedriver" | grep -o '"name":"[^"]*"' | head -1
```

Expect `io.github.hridaya423/phonedriver`. Errors: "Registry validation failed for package" means the published npm package lacks `mcpName` (publish a new patch version; npm versions cannot be replaced); "Invalid or expired Registry JWT token" means log in again.

## 5. After the release: make the docs true

README Install section and `site/content/docs/install.md` say nothing is published yet. Remove that wording, set `RELEASED` in `site/build.py`, and:

```sh
python3 site/build.py && python3 site/check.py
git checkout site/public/assets/fonts          # the build rewrites the fonts; keep the committed ones
```

## 6. Directory submissions

Texts and forms are in `../launch/directories.md`; the order and timing are in `../launch/plan.md`. Do them after step 4, in this order, and check each by searching the directory for `phonedriver` a day later.

1. Glama: claim at https://glama.ai (needs `glama.json`; see directories.md section 2).
2. punkpeye/awesome-mcp-servers: open the pull request (needs the public repo).
3. mcpservers.org: the free form.
4. mcp.so: the issue, if the template exists.
5. "Add to Cursor" link on the site and README (T-3 in the plan).
6. T+2: claudemarketplaces.com and mcpmarket.com (free form).

PulseMCP and Zed pick up registry entries by themselves; check them a few days after step 4.
