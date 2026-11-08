---
title: How to keep an AI agent from spending your money on a phone
nav: Approval guard
meta: phonedriver holds taps on Pay, Place order and Delete account and asks you first. How it works, how to answer, full autonomy, and what it misses.
description: phonedriver stops taps on Pay, Place order and Delete account and asks you first. How the guard works, how to answer, how to switch it off for full autonomy, and what it does not catch.
---

An agent that can drive any app can place an order. phonedriver's approval guard classifies the element the agent is about to tap and, for a button that spends money or cannot be undone, asks the human instead of tapping. It is on by default. The reasoning is in [Letting Claude order dinner without letting it spend your money](/blog/letting-claude-order-dinner-without-letting-it-spend-your-money/).

## What is guarded

The guard reads the tapped element's label. It matches whole-label phrases such as `Pay`, `Pagar`, `Jetzt kaufen`, `提交订单`, `Pay now`, `Buy now`, `Place order`, `Confirm payment`, `Send money`, `Transfer`, `Start subscription` and `Delete account`, and short labels that start with a money verb and hold an amount, such as `Place order · $34.10`, `Send $20.00 to Sam` or `Subscribe · $9.99/month`.

It covers `tap` by ref, by text or by point, `type --submit` when a guarded button sits in the field's own form, and every step of a `batch`.

Never guarded: tabs, menu items, text fields and switches; labels with `#` (an order number); `Add to cart`, `Buy again`, `Pay later`, a bare `Checkout`, a bare `Subscribe` and `Order status`. A chat `Send` is not guarded on purpose: it is the most common tap in the apps agents use, and a prompt on every reply would teach you to click Allow without reading.

## What you see

Over MCP, a client that supports elicitation shows one form that names the app and the button and lists up to eight lines of visible text with an amount or a word like total, deliver, address, item or quantity. The answers are:

- Allow: the tap runs in the same tool call, and the reply says `approved by the human`.
- Allow for this session: the same, and later guarded taps in this session also run. The flag lives in the daemon and is never written to disk.
- Decline or cancel: nothing is tapped. A decline replies `The human declined`.

Clients on protocol 2026-07-28 (Claude Code 2.1.319 speaks it) get the question as an `input_required` result and call the tool again with your answer. Older protocols get `elicitation/create` over the open connection. The agent never sees your answer as something it can supply itself.

If your client cannot ask, the tool replies `Needs the human's approval; nothing was tapped` with the screen facts. Headless `claude -p` has nobody to ask and answers cancel, so the tap does not run.

## Full autonomy

Switch the guard off only when you have decided the agent may spend money or delete things:

- Set `PHONEDRIVER_GUARD=off` in the environment of `phonedriver mcp` or the CLI.
- On the CLI, add `--autonomous` to a single command. Exit code 5 means a tap was held for approval.

`PHONEDRIVER_GUARD` belongs in the MCP server's environment, which the agent does not write.

## What it does not catch

- A fixed list of labels in English, Spanish, Portuguese, French, German, Italian, Dutch, Japanese, Chinese (Simplified), Korean and Hindi. A button in another language, or an icon with no label, is not guarded. A missing prompt is not evidence a tap is safe.
- A tap on a container that merges the label of a guarded child is guarded only when the merged label matches.
- `shell` is out of scope: `input tap` through the shell is not seen.
- An agent can pass `autonomous: true` over MCP, or add `--autonomous` on the CLI, when you never granted it. The guard stops a model that follows the rules or is steered by screen text. It does not stop a model set on bypassing it.
- Point taps in a row reuse the screen the last check read, for up to one second. A change smaller than a significant repaint that adds a guarded button inside that second is missed.
- The form has no screenshot: MCP elicitation has no image field.
