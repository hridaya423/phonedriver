---
title: How to keep an AI agent from spending your money on a phone
nav: Approval guard
meta: phonedriver holds taps on Pay, Place order and Delete account and asks you first. How it works, how to answer, full autonomy, and what it misses.
description: phonedriver stops taps on Pay, Place order and Delete account and asks you first. How the guard works, how to answer, how to switch it off for full autonomy, and what it does not catch.
---

An agent that can drive any app can place an order. phonedriver's approval guard classifies the element the agent is about to tap and, for a button that spends money or cannot be undone, asks the human instead of tapping. It is on by default. The reasoning is in [Letting Claude order dinner without letting it spend your money](/blog/letting-claude-order-dinner-without-letting-it-spend-your-money/).

## What is guarded

The guard reads the tapped element's label, in English, Spanish, Portuguese, French, German, Italian, Dutch, Japanese, Chinese (Simplified), Korean and Hindi. It matches whole-label phrases such as `Pay`, `Pagar`, `Jetzt kaufen`, `提交订单`, `Pay now`, `Buy now`, `Place order`, `Confirm payment`, `Send money`, `Transfer`, `Start subscription` and `Delete account`, and short labels that start with a money verb and hold an amount, such as `Place order · $34.10`, `Send $20.00 to Sam` or `Subscribe · $9.99/month`.

It covers `tap` by ref, by text or by point, `type --submit` when a guarded button sits in the field's own form, and every step of a `batch`.

Never guarded: tabs, menu items, text fields and switches; labels with `#` (an order number); `Add to cart`, `Buy again`, `Pay later`, a bare `Checkout`, a bare `Subscribe` and `Order status`. A chat `Send` is not guarded unless you ask for it (below): it is the most common tap in the apps agents use, and a prompt on every reply would teach you to click Allow without reading.

## Asking about more

Payments and deleting an account are always asked about. Three more kinds of tap are off until you turn them on with `PHONEDRIVER_ASK`, a comma list in the environment of `phonedriver mcp` or the CLI:

| Name | Held taps |
|---|---|
| `send` | `Send`, `Send message`, `Reply`, `Reply all`, `Post`, `Publish`, `Share`, `Forward` |
| `delete` | `Delete`, `Delete message`, `Remove`, `Erase`, `Clear all`, `Clear history`, `Discard changes` |
| `install` | `Install`, `Uninstall` |

For example, `PHONEDRIVER_ASK=send,delete`. The labels are read in the same eleven languages. The prompt names the kind of tap that triggered it (`Category: send`). A name that is not on the list stops the server or command at start with exit code 2, so a typo cannot leave a kind unguarded. `Allow for this session` covers the kind it was given for, not the others. `PHONEDRIVER_GUARD=off` and `autonomous: true` skip these questions too.

## Password and code fields

The guard has one more rule about text. `type` with plain text into a password field, or into a text field whose label, hint or id names a one-time code (`Verification code`, `OTP`, `验证码`, `認証コード`, `인증번호` and the like), is refused with `is a password field` or `is a one-time code field`, and nothing is typed. The error tells the agent to get the text from you and pass it with `secret` (`type --secret` on the CLI, with the text on stdin). Text typed that way is masked and never reaches logs, traces or replies. Clearing such a field is not refused. A plain `Promo code` or `Zip code` field is not affected. phonedriver reads no input type or autofill hint from the phone, only the words, so a code field with no such words is not caught.

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
