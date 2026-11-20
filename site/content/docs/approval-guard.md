---
title: How to keep an AI agent from spending your money on a phone
nav: Approval guard
meta: phonedriver holds taps on Pay, Place order, and Delete account and asks you first. How it works, PHONEDRIVER_ASK options, full autonomy, and blind spots.
description: phonedriver pauses taps on Pay, Place order, and Delete account and asks you directly via MCP elicitation. How it decides, how to tune it, and what it misses.
updated: 2026-11-20
---

On by default: before any tap touches the phone, the approval guard checks the target's label. If it looks like spending money or deleting an account, phonedriver freezes the tap and asks you directly. (For the design story and Domino's checkout walkthrough, see [Ordering dinner without spending your money](/blog/letting-claude-order-dinner-without-letting-it-spend-your-money/).)

## What is guarded by default

The guard reads button labels across eleven languages—English, Spanish, Portuguese, French, German, Italian, Dutch, Japanese, Chinese (Simplified), Korean, and Hindi—and catches:

- Whole-label phrases like `Pay`, `Pay now`, `Buy now`, `Place order`, `Confirm payment`, `Send money`, `Transfer`, `Start subscription`, `Delete account`, `Pagar`, `Jetzt kaufen`, and `提交订单`.
- Labels starting with `Buy with` (such as `Buy with GPay`).
- Short labels (≤ 9 words) starting with a money verb and carrying an amount: `Place order · $34.10`, `Send $20.00 to Sam`, `Subscribe · $9.99/month`.

It covers `tap` (by ref, text, or coordinates), `type --submit` when a guarded button lives in the same form, and every step of a `batch`.

Skipped on purpose so you don't drown in prompts: tabs, switches, text fields, menu items, order numbers containing `#`, `Add to cart`, `Buy again`, `Pay later`, a bare `Checkout`, a bare `Subscribe` (YouTube's is free), `Order status`, and plain chat `Send`.

## Guarding more actions (`PHONEDRIVER_ASK`)

Payments and account deletion are always guarded. To opt into prompts for messaging, deletion, or app installs too, set `PHONEDRIVER_ASK` as a comma-separated list in the MCP server or CLI environment:

| Category | Held button labels |
|---|---|
| `send` | `Send`, `Send message`, `Reply`, `Reply all`, `Post`, `Publish`, `Share`, `Forward` |
| `delete` | `Delete`, `Delete message`, `Remove`, `Erase`, `Clear all`, `Clear history`, `Discard changes` |
| `install` | `Install`, `Uninstall` |

Example: `PHONEDRIVER_ASK=send,delete`. Every category works across the same eleven languages, and unknown category names fail immediately at startup (exit code 2) so a typo can't silently leave you unguarded. Choosing **Allow for this session** only unlocks the category that prompted you.

## Passwords and one-time codes

Plain `type` into a password field—or any field labeled like a verification/OTP code (`Verification code`, `OTP`, `验证码`, `認証コード`, `인증번호`)—is refused automatically. The error tells the agent to pass the text with `secret` (`type --secret` via stdin on the CLI), which masks the value and keeps it out of replies, logs, and traces. Clearing the field is still allowed, and `Promo code` / `Zip code` fields aren't affected. (Detection checks field words only, so an unlabeled OTP box won't trigger it.)

## What you see when a tap is held

In MCP clients that support elicitation (like Claude Code), a native prompt pops up showing the app, button label, and up to eight lines of nearby screen text with totals, items, or delivery addresses. Your three options:

- **Allow** — re-resolves the target against a fresh screen and runs the tap in the same tool call.
- **Allow for this session** — unlocks guarded taps in this category until the daemon exits (RAM only, never written to disk).
- **Decline** — touches nothing and tells the agent `The human declined`.

In clients without elicitation (or headless `claude -p`), nothing is tapped and the tool replies `Needs the human's approval; nothing was tapped` along with the screen details so the agent can ask you in chat.

## Switching it off (full autonomy)

Only disable the guard when you explicitly want the agent acting without confirmation:

- Set `PHONEDRIVER_GUARD=off` in the MCP server's environment (where the agent cannot modify it).
- Pass `--autonomous` on a single CLI command (CLI exits with code 5 whenever a tap is held).

## What it does not catch

- **Fixed list, eleven languages.** Unlabeled icon buttons, other languages, or unusual button copy won't match. No prompt is never proof a tap is safe.
- **Not a security boundary.** An agent can pass `autonomous: true` over MCP or `--autonomous` on the CLI without asking. The guard stops cooperative models and screen-text prompt injections, not an adversarial model bypassing it.
- **Merged containers.** Tapping a parent container only matches if the merged label itself matches.
- **Shell bypass.** `input tap` via `phonedriver shell` doesn't pass through the tap classifier.
- **One-second point-tap cache.** Rapid point taps reuse the previous guard tree read for up to 1 second (cleared on window changes or large repaints) to avoid queuing behind Android button ripples.
- **Text only.** MCP elicitation doesn't support images yet, so prompts show text lines without a screenshot.

## Questions

### Can an AI agent make a purchase with phonedriver?

Not without your click, unless you turned the guard off or authorized autonomous mode. Recognized payment taps pause and display the app, button, and visible amounts first.

### What happens if my MCP client cannot show the prompt?

Nothing is tapped. phonedriver returns `Needs the human's approval; nothing was tapped` with the order details so the agent asks you in chat. In headless `claude -p` nobody can answer, so guarded taps stay blocked and nothing is tapped.

### Does the guard work on an iPhone?

Yes—it inspects the target element's label on both Android and iOS. Verified on a physical iPhone in 0.1.4: held, declined, and unanswerable taps on Place order reached the app zero times.

### Is the guard a security boundary?

No. Think of it as a seatbelt: it catches normal sessions and prompt-injected text, but won't stop an agent that calls `autonomous: true` or taps an unlabeled icon. Use an emulator or spare phone for unattended runs.
