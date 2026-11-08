---
title: Letting Claude order dinner without letting it spend your money
short: Ordering dinner without spending your money
meta: How phonedriver's approval guard stops an agent's tap on Place order or Pay and asks you first, what it covers, and where it stops working.
description: How phonedriver's approval guard stops an agent's tap on Place order or Pay and asks you first through MCP elicitation, what it covers, what it does not, and why the limits are in the design record.
date: 2026-11-07
label: DESIGN
short_card: Letting Claude order dinner without letting it spend your money
card_sub: The approval guard: how it works, and where it stops working.
---

Give an agent a phone and it can do whatever you can do on the screen. That includes tapping Place order. The baseline safety story is a paragraph in the instructions file that says to get the user's word before spending money. It works until the model skips the rule, or a line of text on the screen tells it to. A rule the model can skip is advice. This post is about the control we put in the tool, the thing the model cannot answer for itself, and about where that control stops.

## The design

Before a tap runs, the daemon classifies the element about to be tapped from its label (or its shown value when it has none). If the label means money moves or something cannot be undone, the tap is held and the human is asked. The agent does not get to answer.

The classifier is a fixed list, on purpose. It matches whole-label phrases such as `Pay`, `Pay now`, `Buy now`, `Place order`, `Confirm payment`, `Send money`, `Transfer`, `Start subscription` and `Delete account`. It also matches a label of at most nine words that starts with a money verb and holds an amount: `Place order · $34.10`, `Send $20.00 to Sam`, `Subscribe · $9.99/month`. Roles that only navigate or edit are never guarded: tabs, menu items, text fields, switches. The guard covers `tap` by ref, by text and by point, `type --submit` when a guarded button sits in the field's own form, and every step of a `batch`, where a guarded step ends the batch.

We rejected a model call to classify labels, for three reasons: it costs tokens on every tap, it sends screen text somewhere (we have a no-telemetry rule), and a model can be talked out of its answer. A fixed list can be wrong, but it cannot be persuaded.

## Who asks, and how

The daemon never asks anyone. It fails the tap with a `needs_approval` error that carries the label, the app and up to eight lines of visible text that hold an amount or a word like total, deliver, address, item or quantity. The CLI exits with code 5 on it. The MCP server turns it into a question for the human.

A client that supports elicitation is sent one form: the message names the app and the button and lists those screen lines. The answer is Allow, Allow for this session, or Decline. Allow runs the tap in the same tool call, and the reply says `approved by the human`. Allow for this session sets a flag on the daemon's session that ends with the daemon and is never written to disk. Decline returns a normal reply, not an error, and nothing is tapped.

Newer clients forbid a server from sending a request in the middle of a call. Protocol 2026-07-28, which Claude Code 2.1.319 speaks, instead has the tool return an `input_required` result with the question, and the client calls the tool again with the answer. Older protocols get `elicitation/create` over the open connection and the call waits. Either way, the agent spends no calls on it: a refusal that the agent must retry would make every purchase two calls and invite `approve: true` by reflex.

After Allow, the target is resolved again, because the screen can change while you decide. A ref the screen no longer has fails as stale instead of tapping something else.

## What it does not guard, on purpose

A chat `Send` is not guarded. It is the most common tap in the apps agents are asked to use, a prompt on every reply would teach you to click Allow without reading, and the instructions already say that a message needs the user's word. `Send $20.00` and `Send money` are guarded. Also not guarded: `Pay later`, `Pay bills`, `Payments history`, `Buy again`, `Add to cart`, a bare `Checkout`, a bare `Subscribe` (YouTube's is free and the most common) and `Order status`.

Every one of those is a judgment call. We wrote the reasoning next to the list, and the test table for the labels is in `crates/cli/src/daemon/guard.rs`. If real use shows labels missed often enough, or false prompts often enough, the list changes.

## The limits, stated plainly

This is the part to read before you hand an agent a phone with a payment app signed in.

- **Fixed list, eleven languages.** The list covers English, Spanish, Portuguese, French, German, Italian, Dutch, Japanese, Chinese (Simplified), Korean and Hindi. A button in another language, or an icon with no label, is not guarded. A prompt that does not appear is not evidence that a tap is safe.
- **A model set on bypassing it can.** Over MCP an agent can pass `autonomous: true`, and on the CLI it can add `--autonomous` or set `PHONEDRIVER_GUARD=off`, when you never granted it. The guard stops a model that follows its rules or is steered by text on the screen. `PHONEDRIVER_GUARD` belongs in the MCP server's environment, which the agent does not write, but the CLI has no human at that terminal to ask.
- **A merged container.** Names are matched on the element tapped. A tap on a container that merges the label of a guarded child is guarded only when the merged label matches.
- **The shell.** `input tap` through `shell` is out of scope. A button pressed by shell input is not seen.
- **A one-second window.** Point taps in a row reuse the screen the last guard check read, for up to a second while no window change or significant repaint arrives. We did this because a read right after a tap queues behind the button's ripple on the app's UI thread: 94 ms at p50 back to back against 33 ms at rest, and a tap reply of 126 ms against 20 ms with the reuse on the API 26 emulator. The cost is that a change smaller than a significant repaint that adds a guarded button inside that second is missed.
- **No image in the question.** MCP elicitation has no image field in any version we checked, so there is no screenshot in the prompt. The prompt shows the screen's visible text to the human on their own machine, and the agent, in the no-elicitation reply, gets the same lines, which it could already read.
- **Nobody home.** Headless `claude -p` has nobody to ask and answers cancel, so the tap does not run there.

## One change that came from running it

An earlier version treated a cancelled form as a refusal. In the jobs benchmark, a prompt that said "you may place the order" ran under `claude -p`, which answers every prompt with cancel because nobody is there. The job ended at a dead stop with no way forward. A cancel is now no answer, not a refusal: only Decline says `The human declined`. A cancel replies `Needs the human's approval; nothing was tapped`, includes the screen facts, and tells the agent the way on: ask in chat, and call again with `autonomous: true` only if the user's own instructions already approve the tap. The guard still taps nothing without an approval. A screen that tries to inject an approval still cannot give one, because `autonomous` stays the agent's call under the written rules.

## Why it lives in the tool

An agent that can tap Pay should not be the thing that decides whether it may. The instructions file still says the right thing, and a careful model follows it. The guard is the part that holds when the model does not, so the question reaches the person where they are, with the facts from the screen, instead of arriving as a chat message the model writes. It is a seat belt, not a lock. We would rather say so than let the first sentence of this post stand alone.

Setup, switches and exit codes are in [the approval guard guide](/docs/approval-guard/). The full decision record, D27, is in [the repository]({{REPO}}/blob/main/docs/decisions.md).
