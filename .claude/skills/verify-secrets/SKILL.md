---
name: verify-secrets
description: Use when a change touches how the daemon handles secret text (D16, D31, D70): daemon/secrets.rs, Daemon::call in daemon/mod.rs, sanitize in daemon/trace.rs, the Type arm of act_now and Session::unsure in actor.rs and session.rs, daemon/journal.rs, the Debug impls of TextInput and ShellRequest, or any new reply field, error or file the daemon writes. Runs a property test of the real daemon over generated sessions and says whether text given with `secret` can come back in a reply, an error, a trace file or the saved steps, and whether plain text can reach a password field.
---

# Verify that secrets stay secret

**Promise.** Text given to the daemon with `secret: true` (the text of a `type`, alone or as a `batch` step, and the command of a `shell`) never appears again: not in the reply to that call, not in any later reply or error to any session, not in a file under `PHONEDRIVER_HOME`, not in the steps `journal` returns for `save`, and not in the `Debug` print of what the device was sent. Text not marked secret never reaches a password field.

**Harm if it breaks.** A person's password, card number or token is printed into an agent's transcript, which is stored and may be sent to a model provider, or is left in a trace file on disk.

## Tool, and why

Property-based testing (proptest) of the real code. Nothing verifies Rust of this shape (async, serde values, trait objects) for every input, so the check runs the real `Daemon` against the real `FakeDevice` with generated sessions; a hand model of the daemon would check the copy. The question is about data, not about the order of events, so a model checker has nothing to add: the daemon serves one call at a time for a device. Nothing to install: `proptest` is a dev-dependency of `crates/cli`.

The oracle shares no code with the daemon. It is two string searches: every secret the test passed, in everything the daemon gave back or wrote; and every text the fake's password field received, against the list of texts passed as secret.

## Coverage record

All checks are in `crates/cli/tests/secret_property.rs`. The fake phone there (`Screen`) has a note field that shows its text and a password field that shows bullets; a tap moves the focus; `deaf` fields ignore text; a failing device rejects one `text` call with a message that prints the text back; a shell command can print every earlier secret back, and so can the web page `page` reads.

| Check | What it drives | Code it covers |
|---|---|---|
| `a_secret_reaches_no_reply_error_or_file_and_plain_text_no_password_field` | 256 sessions of 1 to 6 steps: `type` (secret or not; by ref, by name or into the focus; submit, append; alone or as a batch step), `shell` (secret or not; output echoing earlier secrets), taps on either field, on nothing or on a label that repeats what the note field holds, `check` of either field with and without a wrong `equals` (D113), `notifications` and a wait for a notification that prints every earlier secret back (D117), `snapshot`, `find`, `journal`, `record start` and `stop`, `page` (a web page that prints every earlier secret back); full traces on or off. After every call it searches the reply or error and every file under the home. At the end it searches the journal and the device's call log, and checks what the password field took. | `daemon/secrets.rs` (all of it). `daemon/mod.rs`: `Daemon::call`. `daemon/trace.rs`: `sanitize`, `hide_action`, `Traces::record`. `daemon/actor.rs`: the `Type` arm of `act_now` (the `secret_field` refusal, `withhold`, the focused-field lookup), `act` (`s.unsure`, and the summary it hands `Taping::step`), `shell`, `batch`. `daemon/session.rs`: `unsure`, `set_last`. `daemon/journal.rs`: `Kept::of`, `Kept::step`. `core/device.rs`: `Debug` for `TextInput` and `ShellRequest`. `core/testing.rs`: `Call`. |
| `the_sessions_that_leaked_before_the_fixes_stay_closed` | The five sessions behind findings F1 to F3 and F5, fixed. F3 needs a device failure at one exact step, which the random property meets about once in 3,000 sessions. | Same. |
| `every_kind_of_step_works_on_the_fake_phone` | One session with every kind of step; asserts three types went in, one check read a value and four secrets were tracked. It fails when the harness stops reaching the code (a renamed parameter makes every call an error, and a session of errors leaks nothing). | The harness. |
| `survey` (ignored) | 3,000 deterministic sessions; prints every distinct kind of leak with a count and the first session that shows it. The property stops at its first failure; this lists them all. | Same as the first row. |

Three unit tests in `daemon/secrets.rs` cover what the property's fixed secrets cannot: a secret inside a longer one, a secret under four characters, and a screenshot's `data_b64` left alone.

**Sync commit: `4810ba8`** (read on 2026-11-16: since `7369a14`, which added the `page` call, the covered files gained the `boot` method, whose reply leaves through `Daemon::call` and which takes no secret, two more buttons, and a locked phone's answer in hidden mode; none takes or writes secret text).

2026-11-24: the `check` method and batch step (D113) were added and are generated by the property (`Step::Check`); break B1 run on the Mac mini failed with `step 1 (check): the reply holds the secret` and the trace file line. Only that change was read, not the whole diff since the sync commit (history was rewritten twice since), so the sync commit above is not moved.

2026-11-24, later: the `notifications` method and the wait for a notification (D117) were added; the property generates both (`Step::Notifications`) against a notification that repeats every earlier secret, and they are in the every-step session. `scroll` with `collect` (D116) returns screen text in `rows` through the same `Daemon::call` and is not generated. The sync commit is still not moved, for the reason above.

2026-11-24, named paths (D118): a run saved under `paths/` ends with a landmark, a text the command line takes from a `snapshot` with `elements` and writes into the steps file. No covered file changed. The property generates that read (`Step::Landmark`), so a secret in an element's label would fail it as a reply that holds the secret; `paths::landmark` also passes over a label that holds the mask and every text field (unit test `typed_text_masked_text_and_a_long_sentence_are_never_the_landmark`). The sync commit is not moved.

When you finish a change, run:

```
git diff --stat 4810ba8 -- crates/cli/src/daemon/secrets.rs crates/cli/src/daemon/mod.rs crates/cli/src/daemon/trace.rs crates/cli/src/daemon/actor.rs crates/cli/src/daemon/session.rs crates/cli/src/daemon/journal.rs crates/cli/src/wire.rs crates/core/src/device.rs crates/core/src/testing.rs
```

Empty output means the check still describes the code. Otherwise read the diff for: a new method or action that takes secret text (add it to `secrets::carried` and to `Step` in the test); a new place the daemon writes (a file, a log line, a new process: F5 was one, and such a place must mask with `Secrets::masked` itself); a reply that leaves by another door than `Daemon::call`; a new string field whose content must not be masked (as `data_b64` is skipped); a change to when plain text is refused. Run the check. When it passes and you have confirmed the generators still reach the changed code, move the sync commit to the new hash. Do not move it only because the test passes: a new secret-carrying call the generators never send passes too.

## Run

```
cargo test --locked -p phonedriver --test secret_property
cargo test --locked -p phonedriver --lib secrets
```

About 2 seconds after the build. `-- --nocapture` prints `coverage:` lines; a good run ends near 160 types that worked, 40 refused or failed, 115 shell commands and 150 secrets. To list every kind of leak instead of the first: `cargo test --locked -p phonedriver --test secret_property survey -- --ignored --nocapture` (about 15 seconds). On the Mac mini there is no blaze; on a machine with it, `blaze test -p phonedriver --test secret_property`.

A failure prints one line per leak and the shrunk session, for example:

```
step 1 (snapshot): the reply holds the secret Ty0mW4zR8nB3c: {"changed":true,…
steps: [Type { flagged: true, into: Note, … }, Snapshot], full trace false, deaf fields false, failing device false
```

Read it as: step 0 typed a secret into the note field, step 1 took a snapshot, and the snapshot's reply printed the secret. `Ty<n>…` is the text typed at step n; `login --token Sh<n>…` is the shell command of step n. `flagged` is `secret: true`. `into` is how the field was named: `Note` and `Password` by ref, `NoteByName` and `PasswordByName` by label, `Focused` with no target.

## Real bug or modelling error

A **real bug** is a line of the form `the reply holds the secret`, `<file> holds the secret`, `the steps save would write hold the secret`, `the device's call log prints the secret`, or `the password field was given text that was not marked secret`. Reproduce it in `crates/cli/tests/daemon.rs` with `FakeSource` and the same calls, and read the reply. If the story reads wrong in the product's terms ("the user's password is in the snapshot the agent was sent"), it is a bug. Report it with the session; do not change the oracle to make it pass.

A **modelling error** is when the fake phone did something no phone can. The two this check has met: the fake printing back a text the daemon never sent it (a refused step's text), and a failure message written before the session that named texts of later steps. A phone can only echo what it was given. Other causes: a new string field that legitimately carries bytes a secret can collide with (skip it in `hide_value`, as `data_b64` is, and say so in D70); a deliberate change to what counts as secret (update D70 in `docs/decisions.md` first, then the oracle, in the same commit).

If `every_kind_of_step_works_on_the_fake_phone` fails, fix the harness first: the other results mean nothing until it passes.

To see the check fail on purpose: `python3 .claude/skills/verify-secrets/breaks.py <name>` applies one break to the product code, runs the check, writes the file back as it was, and saves the output under `target/verify-secrets/`. Run it with no argument for the names.

The script writes the file back with a new modification time, which is what makes cargo build it again. If you then copy the tree to another machine with `rsync -a` (or anything that sets times back), cargo there keeps the broken build and the check fails on correct code: `touch` the file on that machine.

| Break | Caught by |
|---|---|
| B1 replies are not masked (`mod.rs`) | property, fixed sessions |
| B2 a secret is forgotten after its own call (`secrets.rs`) | property, fixed sessions |
| B3 a batch step's secret is not read | property, fixed sessions |
| B4 a shell command's secret is not read | property |
| B5 the trace keeps typed text (`trace.rs`) | nothing since `Traces::record` also masks its params with the remembered secrets (seen 2026-11-24: the break passes); it guards text that was not marked secret, which this check does not search for |
| B6 the trace keeps a secret command | property |
| B7 `Debug` prints typed text (`device.rs`) | property, fixed sessions |
| B8 `Debug` prints a shell command | property |
| B9 a failed step leaves the old focus trusted (`actor.rs`) | fixed sessions only |
| B10 plain text goes into a password field | property, fixed sessions |
| B11 the steps file of a recording is not masked (`actor.rs`) | fixed sessions |

Before a commit that touches the covered files, run at least one break in the area you changed and confirm it still fails. A check that cannot fail is dead.

## Findings, fixed

Found on the first run, 2026-11-15, and fixed in the sync commit (decision D70 has the detail):

- **F1.** A secret typed into a field that shows its text was masked in that one reply, then printed by the next `snapshot`, `find` or action reply, and written to the trace under `--trace-full`.
- **F2.** A secret `type` as a `batch` step was not masked at all. Only a client other than the CLI and MCP could send one.
- **F3.** After a secret `type` whose tap moved the focus to the password field and whose text then failed, the next plain `type` with no target trusted the old screen and pasted plain text into the password field.
- **F4.** `FakeDevice` kept a shell command as a `String`, so a failed assertion printing the call log printed a `shell --secret` command.
- **F5.** Found by a review, not by this check, the day it was written: the steps file of a recording is written inside the actor, before `Daemon::call` masks anything, so a tap on a label that showed a remembered secret wrote the secret to `<video>.steps.jsonl`. The fake phone had no such label then. It has one now (`TapWhatTheNoteSays`).

## Not covered

- Pixels: a screenshot, a recording, and the screenshot copies of `--trace-screens` show what the phone shows.
- A secret under four characters after its own call; a secret the device prints back changed (escaped, wrapped, cut short); a later call that itself carries the same text without `secret`.
- The CLI and the MCP server: how they read the secret (stdin, `PHONEDRIVER_SECRET`), their own error text, the refusal of a secret on argv and in a batch on the command line, and how the MCP server passes a `type` step's `secret` in a `batch` or before a `then`. `tests/e2e.rs` and `tests/mcp.rs` have example tests for those; the daemon's side of a secret batch step is in the property.
- The on-device agent (Java) and what Android or iOS log about a paste; the clipboard after a secret paste.
- `clipboard`, `logs`, `setting`, `wait`, `app` and `screenshot` calls are not generated. Their replies leave through the same `Daemon::call`, so the mask applies, but no session here proves it.
- More than one device, a daemon restart (the list of secrets is in memory only), two calls at once.
- Bounds: sessions of at most 6 steps, secrets of 13 and 26 characters, two fields, one screen. A pass covers those.
