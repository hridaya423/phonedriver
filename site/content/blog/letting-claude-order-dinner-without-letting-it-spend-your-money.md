---
title: Letting Claude order dinner without letting it spend your money
short: Ordering dinner without spending your money
meta: How phonedriver holds taps on Place order or Pay and asks you through MCP elicitation—why it's a fixed list, not an LLM, and where it stops working.
description: Claude built a Domino's pizza order near Leicester Square, aimed at Buy with GPay, and got stopped at the door. How the approval guard works, and why it's a seatbelt rather than a vault.
date: 2026-11-07
label: DESIGN
short_card: Letting Claude order dinner without letting it spend your money
card_sub: How phonedriver stops a tap on Pay, and where it stops working.
updated: 2026-11-20
---

I told Claude Code to order a large Pepperoni Passion from the Domino's nearest Leicester Square, for collection, as a guest, and explicitly told it *not* to ask before placing the order.

Six and a half minutes later, Opus 5.5 had tracked down the store, customized the pizza, survived Domino's mobile checkout, filled in guest details, and aimed its finger squarely at **`Buy with GPay`**.

Nothing happened. phonedriver froze the tap in mid-air, popped a prompt straight into Claude Code's UI naming the app, the button, and the order details, and waited for me. I clicked **Decline**. Claude gracefully reported that no pizza was ordered and nothing was charged. ([The 43-second demo video]({{REPO}}/blob/main/.github/assets/demo.mp4) plays the whole run at 15x speed up to checkout, then drops to real speed for the standoff.)

That is the approval guard. It keeps an overeager agent, or a sneaky prompt injection buried in a webpage, from tapping your card. It is *not* a cryptographic vault, and it is worth knowing both how it works and every way past it.

## Why a dumb list beats a second LLM

Before a tap touches the phone, the daemon classifies the element's label (or its visible value if it has no label). Recognized payment and account-deletion labels stop cold.

Asking a small fast model whether a button looks risky sounds clever until you price it out. It would burn tokens on every tap, ship your screen text off your machine (breaking phonedriver's no-telemetry rule), and hand a hostile webpage a brand-new prompt-injection target sitting right inside the safety check.

So the guard is a plain deterministic list in `crates/cli/src/daemon/guard.rs` covering eleven languages, paired with one short pattern rule: up to nine words starting with a money verb and carrying an amount (`Place order · $34.10`, `Send $20.00 to Sam`, `Subscribe · $9.99/month`). Roles that only navigate or edit—tabs, switches, text fields, menu items—never get held.

::figure[guard-playground]

## Who asks, and why the model never sees the form

The daemon itself never talks to a human. It just rejects the tap with a `needs_approval` error (CLI exit code 5) carrying the app name, the button label, and up to eight lines of nearby screen text mentioning totals, addresses, items, or quantities.

The MCP server turns that error into a native prompt inside your client via MCP elicitation. You choose **Allow**, **Allow for this session** (kept in daemon RAM only, never written to disk), or **Decline**. Click Allow, and phonedriver re-resolves the target against a fresh screen—in case the page shifted while you were reading—and finishes the tap inside the same tool call.

::figure[elicitation-circuit]

Keeping the prompt inside a single tool call matters. If the tool refused and forced the agent to ask you in chat and retry, every purchase would turn into a two-turn ritual that trains both humans and models to rubber-stamp `approve: true` on autopilot.

## What I left unguarded on purpose

The fastest way to ruin a safety prompt is to fire it on every message. A plain chat **`Send`** button is unguarded by default; interrupt someone on every text reply and they'll be clicking Allow from muscle memory by noon. `Send $20.00` and `Send money`, on the other hand, are always held.

Also skipped by design: `Add to cart`, `Buy again`, `Pay later`, `Payments history`, `Order status`, a bare `Checkout`, and a bare `Subscribe` (YouTube's is free and ubiquitous). If you *do* want prompts before sending messages, deleting items, or installing apps, opt in with `PHONEDRIVER_ASK=send,delete,install` ([approval guard guide](/docs/approval-guard/)).

## The bug headless runs exposed

Version one treated a **cancelled** elicitation form identically to **Decline**. That seemed tidy until a benchmark job told headless `claude -p` *"you may place the order"*. Nobody sits in front of `claude -p`, so the client automatically cancels every form—and phonedriver told Claude *"The human declined"*, stranding the job on the spot.

Now cancel means *nobody answered*, not *no*. Only an explicit Decline returns `The human declined`. A cancelled form replies `Needs the human's approval; nothing was tapped` along with the screen facts, letting the agent ask in chat—or retry with `autonomous: true` only when the user's own instructions already granted permission.

<span id="the-limits-stated-plainly"></span>

## What the guard misses

The guard stops an agent playing by the rules or getting nudged by untrusted text on screen. It will **not** stop a model determined to evade it, and it has six plain blind spots:

- **Eleven languages, text labels only.** English, Spanish, Portuguese, French, German, Italian, Dutch, Japanese, Chinese (Simplified), Korean, and Hindi. Another language, creative copy like *"Feed me now"*, or an unlabeled icon button sails straight past. No prompt does not mean safe.
- **`autonomous: true` skips the check.** An agent can pass `autonomous: true` over MCP (or `--autonomous` on the CLI) even when you never authorized it. `PHONEDRIVER_GUARD=off` lives in the MCP server's environment where the agent can't edit it, but the per-call flag is still trust-based.
- **Merged containers.** Tapping a parent layout whose merged label doesn't match a guarded phrase won't trigger the guard.
- **The `shell` tool.** Raw `input tap` via shell bypasses the tap classifier completely.
- **A one-second cache window.** Back-to-back point taps reuse the previous guard read for up to a second (cleared on window changes or large repaints) so taps don't queue behind Android's button ripple. A tiny repaint that inserts a Pay button inside that second is missed.
- **No screenshot in the prompt.** MCP elicitation has no image field yet, so the prompt shows screen text lines only.

## A seatbelt, not a vault

A line in a system prompt saying *"ask before buying anything"* can drift out of attention twenty turns in, or get sweet-talked away by text on a webpage. Putting the brake pedal in the tool makes sure a normal session actually stops at `Buy with GPay` and shows you the total first.

Treat it like a seatbelt: wear it, appreciate it, and still don't leave an unattended agent alone with your primary phone and Apple Pay unlocked.

:::receipts References & configuration
- **MCP protocol note:** Clients on `2026-07-28` (including Claude Code 2.1.319) use `input_required` and replay the tool call with your answer; older clients use `elicitation/create` over the open connection.
- **Setup, switches & exit codes:** [Approval guard guide](/docs/approval-guard/).
- **Full design record:** [Decision D27]({{REPO}}/blob/main/docs/decisions.md) and label tests in `crates/cli/src/daemon/guard.rs`.
:::
