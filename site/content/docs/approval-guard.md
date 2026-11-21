---
title: How to keep an AI agent from spending your money on a phone
nav: Approval guard
meta: phonedriver holds taps on Pay, Place order and Delete account and asks you first. How it works, how to answer, full autonomy, and what it misses.
description: phonedriver stops taps on Pay, Place order and Delete account and asks you first. How the guard works, how to answer, how to switch it off for full autonomy, and what it does not catch.
updated: 2026-11-21
---

The guard classifies the element the agent is about to tap and, for a button that spends money or cannot be undone, asks the human instead of tapping. It is on by default. The reasoning is in [Letting Claude order dinner without letting it spend your money](/blog/letting-claude-order-dinner-without-letting-it-spend-your-money/).

## What is guarded

The guard reads the label of the element the agent is about to touch, in English, Spanish, Portuguese, French, German, Italian, Dutch, Japanese, Chinese (Simplified), Korean and Hindi. It matches whole-label phrases such as `Pay`, `Pagar`, `Jetzt kaufen`, `提交订单`, `Pay now`, `Buy now`, `Place order`, `Confirm payment`, `Send money`, `Transfer`, `Start subscription` and `Delete account`, labels that start with `Buy with` or `Pay with`, such as `Buy with GPay`, sliders such as `Slide to pay` or `Swipe to confirm`, and short labels that start with a money verb and hold an amount, such as `Place order · $34.10`, `Pay 50 kr` or `Subscribe · $9.99/month`.

It covers every way an agent can press such a button:

- `tap` by ref, by text or by point, and a tap on a price or icon inside a guarded row.
- `swipe`, `drag`, `pinch` and `gesture` when the finger goes down and lifts on the button, which the phone treats as a click; a drag that drops onto it; and any stroke along a `Slide to pay` control.
- `type --into` and `clear` aimed at the button.
- `type --submit` and `press enter` when the field's form holds a guarded button, even one the form only turns on once the field is filled.
- Every step of a `batch`.

An ordinary swipe, scroll or pinch is never asked about, and costs no extra time.

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

- Allow: the action runs in the same tool call, and the reply says `approved by the human`. If the button or the amounts on screen changed in the meantime, you are asked again.
- Allow for this session: the same, and later guarded taps in this session also run. The flag lives in the daemon and is never written to disk.
- Decline or cancel: nothing is tapped. A decline replies `The human declined`.

Clients on protocol 2026-07-28 (Claude Code 2.1.319 speaks it) get the question as an `input_required` result and call the tool again with your answer. Older protocols get `elicitation/create` over the open connection. The agent never sees your answer as something it can supply itself.

In the demo, one real Claude Code session (Opus 5.5) orders a large pepperoni pizza on dominos.co.uk on an Android emulator. It reaches Buy with GPay, phonedriver holds the tap, and the human declines. Nothing is ordered.

If your client cannot ask, the tool replies `Needs the human's approval; nothing was tapped` with the screen facts. Headless `claude -p` has nobody to ask and answers cancel, so the tap does not run.

## Full autonomy

Switch the guard off only when you have decided the agent may spend money or delete things:

- Set `PHONEDRIVER_GUARD=off` in the environment of `phonedriver mcp` or the CLI.
- On the CLI, add `--autonomous` to a single command (`tap`, `type`, `swipe`, `drag`, `pinch`, `gesture`, `scroll`, `press` or `clear`). Exit code 5 means the action was held for approval.

`PHONEDRIVER_GUARD` belongs in the MCP server's environment, which the agent does not write.

## What it does not catch

- A fixed list of labels in the eleven languages above. A button in another language, or an icon with no label, is not guarded. A missing prompt is not evidence a tap is safe.
- A tap on a container that merges the label of a guarded child is guarded only when the merged label matches.
- `shell` is out of scope: `input tap` through the shell is not seen.
- An agent can pass `autonomous: true` over MCP, or add `--autonomous` on the CLI, when you never granted it. The guard stops a model that follows the rules or is steered by screen text. It does not stop a model set on bypassing it.
- Point taps in a row reuse the screen the last check read, for up to one second. A change smaller than a significant repaint that adds a guarded button inside that second is missed.
- A swipe or drag is checked against the screen the agent last saw. A guarded button that appeared since, under the finger, is missed.
- `press fingerprint` on an emulator answers a fingerprint prompt, which an app can use to confirm a payment. It is not guarded.
- The form has no screenshot: MCP elicitation has no image field.

## Questions

### Can an AI agent make a purchase with phonedriver?

Not without your answer, unless you switched the guard off or told the agent to run autonomously. A tap on a button such as `Pay`, `Buy now` or `Place order` is held, and you see the app, the button and the amounts on screen before anything is tapped.

### What happens if my MCP client cannot show the prompt?

The tool replies `Needs the human's approval; nothing was tapped` with the facts from the screen. The agent then has to ask you in chat. In headless `claude -p` nobody can answer, so nothing is tapped.

### Does the guard work on an iPhone?

Yes. It reads the label of the element about to be tapped, on Android and on iOS. In release 0.1.4 it was tested on a physical iPhone: a held, declined or unanswerable tap on Place order reached the app zero times.

### Is the guard a security boundary?

No. It stops a model that follows its rules, and one that is steered by text on the screen. It does not stop a model set on bypassing it, and it does not see a button that is only an icon or a tap sent through the shell. For unattended runs, use an emulator or a spare phone with no payment method on it.
