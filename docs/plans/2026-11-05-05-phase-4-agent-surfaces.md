# Phase 4: Agent surfaces

> **For Claude:** REQUIRED SUB-SKILL: Use `executing-plans` to implement task-by-task. Load `prose` for SKILL.md and README text.

**Goal:** Agents use phone-use well with no human help: an MCP server, a skill file, `doctor`, traces, and error text tuned on real agent runs. Milestone **M3**.

**Architecture:** `docs/architecture.md` sections 5 (traces), 10.2, 10.3, 13.

**Tech Stack:** `rmcp` 3.9 (stdio server; 3.8 was current when this was written), Claude Code headless (`claude -p`) for the evaluation.

---

### Task 4.1: MCP server

**Files:** Create `crates/cli/src/mcp.rs`; test `crates/cli/tests/mcp.rs`.

**Step 1: Failing tests** with an in-process MCP client from `rmcp` against `phone-use mcp` running on a daemon with `FakeDevice`:
- `tools/list` returns exactly the ten tools in architecture 10.2, each with a description under 300 characters and a JSON schema whose required fields match the CLI's required arguments.
- `tools/call snapshot` returns one text content block equal to the CLI output for the same state.
- `tools/call screenshot` returns one image content block (`image/png`, base64) and one text block with size and scale.
- An error (`stale_ref`) returns `isError: true` and the message plus hint as text.

**Step 2:** FAIL. **Step 3:** Each tool calls the same daemon RPC as the CLI verb. No logic in the MCP layer beyond schema mapping. **Step 4:** PASS. Commit.

### Task 4.2: SKILL.md and `phone-use skill`

**Files:** Create `SKILL.md` (frontmatter `name: phone-use`, description that triggers on "phone", "iPhone", "Android", "tap", "mobile app"). `phone-use skill` prints it (embedded with `include_str!`).

Content, under 150 lines, in this order: the loop (snapshot, act on refs, read the diff, act again); targets (prefer refs; text targets for known labels; points only from screenshots); waiting (`wait` for a condition, never sleep); when to use `--pixels` and `screenshot`; handling `needs_user` (stop and tell the user exactly what the error says); safety (never type passwords, codes or payment details unless the user gave them for this task; never confirm purchases or send messages to people without the user's explicit instruction).

Test: `phone-use skill` output equals the file (CLI test). Review the text with the `prose` skill.

### Task 4.3: `doctor`

**Files:** Create `crates/cli/src/doctor.rs`.

Checks, each printing `ok`, `warn`, or `fail` with a fix: adb server reachable (and which address); each Android device's state (`unauthorized` → "accept the USB debugging prompt on the phone"); agent deploys and answers `hello`; UiAutomation connects (conflict → name the holder); video frames arrive within 1 s; daemon socket permissions. iOS checks are added in Phases 5 and 6.

Test: with the emulator, all `ok`. With the adb tunnel down, `fail` on the adb line with the `ADB_SERVER_SOCKET` it tried.

### Task 4.4: Traces

**Files:** Modify `crates/cli/src/daemon/session.rs`; create `crates/cli/tests/trace.rs`.

**Tests:** after a session with a `type --secret`, the trace file has one JSON line per request with `method`, `params` (secret replaced by `{"secret_len":7}`), the reply's summary line, `ms`. No line contains the secret (property test over random secrets), including when typed into a `SecureField` without the flag. Observation text appears only with `--trace-full`. `--trace-screens` writes PNGs next to the trace. Trace directory mode 0700. A trace file dated 8 days ago is deleted on daemon start.

### Task 4.5: Agent evaluation (M3)

**Files:** Create `bench/agent-eval/tasks.yaml`, `bench/agent-eval/run.sh`, `docs/bench/m3.md`.

Ten tasks in natural language, each with a programmatic check reused from the scenario suite, split into two sets. Do not look at the held-out set's transcripts until the gate run.

- **Tuning set** (iterate on these): turn on dark theme; set an alarm for 7:30; in the testbed, fill the web form and submit; scroll the testbed list to Row 640 and open it; find the testbed notification's code and type it into the testbed field.
- **Held-out set** (gate only): turn on airplane mode then turn it off; create a folder named "phone-use" in Files; in Chrome, open example.com and report the page heading; in the testbed, toggle the third switch on the Compose tab; set a 3-minute timer in Clock.

Runner: for each task, reset the emulator state, then run `claude -p "<task>" --mcp-config bench/agent-eval/mcp.json --allowedTools "mcp__phone-use__*" --model claude-sonnet-5-5 --output-format stream-json`, save the transcript, run the check.

Record per task: success, number of tool calls, total tokens, wall time. Read every tuning-set transcript. For each place the agent hesitated, misused a verb, or misread an error, improve the tool description, the error hint, or the snapshot format, then rerun the tuning set.

**Gate M3:** on the held-out set, run each task 3 times (15 runs): at least 13/15 succeed, and no failure is caused by the harness (wrong tap, missed element, misleading error). Classify every failure by reading its transcript. Record all metrics, the changes made during tuning, and their before/after effect on tool calls and tokens in `docs/bench/m3.md`.

### Task 4.6: README

**Files:** Create `README.md`. Under 120 lines: what it is in one sentence, a 6-line demo (the loop from architecture section 1), install from source, supported devices table with tiers, MCP setup snippet, link to `SKILL.md`, the M1 benchmark table. Load `readme` and `prose`. No badges, no marketing images.

---

## Phase gate

M3 met. `phone-use doctor` all `ok` on the emulator. `progress.md` updated.
