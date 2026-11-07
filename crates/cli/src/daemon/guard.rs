//! The approval guard (decision D27): which taps spend money or cannot be undone, and what to tell the human.

use phone_use_core::element::{Element, ElementState, Role};
use phone_use_core::geometry::Point;
use phone_use_core::render::Rendered;

use crate::daemon::session::Session;
use crate::wire::Approval;

/// What to do with a tap that the classifier guards.
#[derive(Debug, PartialEq, Eq)]
pub enum Stance {
    Run(Option<&'static str>),
    Ask,
}

pub fn stance(s: &mut Session, approval: Option<Approval>) -> Stance {
    match approval {
        Some(Approval::Autonomous) => Stance::Run(None),
        Some(Approval::Session) => {
            s.guard_allowed = true;
            Stance::Run(Some("approved by the human for this session"))
        }
        Some(Approval::Once) => Stance::Run(Some("approved by the human")),
        None if s.guard_allowed => Stance::Run(Some("approved by the human for this session")),
        None => Stance::Ask,
    }
}

const EXACT: &[&str] = &[
    "pay",
    "pay now",
    "buy",
    "buy now",
    "purchase",
    "checkout and pay",
    "pay and place order",
    "place order",
    "place your order",
    "place my order",
    "place your order and pay",
    "confirm payment",
    "confirm and pay",
    "confirm purchase",
    "confirm order",
    "complete purchase",
    "complete order",
    "complete payment",
    "order now",
    "send money",
    "transfer",
    "transfer money",
    "transfer now",
    "confirm transfer",
    "delete account",
    "delete my account",
    "delete your account",
    "permanently delete account",
    "start subscription",
    "confirm subscription",
    "subscribe and pay",
];

const PREFIX: &[&str] = &[
    "buy now",
    "buy with",
    "pay now",
    "place order",
    "place your order",
];

const AMOUNT_VERBS: &[&str] = &[
    "pay",
    "buy",
    "purchase",
    "checkout",
    "subscribe",
    "send",
    "transfer",
    "donate",
    "order",
    "confirm",
    "place",
    "complete",
];

const CODES: &[&str] = &[
    "usd", "eur", "gbp", "cad", "aud", "inr", "jpy", "mxn", "chf", "cny",
];

const SYMBOLS: &[char] = &['$', '€', '£', '¥', '₹'];

pub fn app_name(now: &Rendered) -> Option<String> {
    now.header
        .app
        .as_ref()
        .map(|a| a.name.clone().unwrap_or_else(|| a.id.clone()))
}

const MAX_WORDS: usize = 9;

fn tokens(text: &str) -> Vec<String> {
    let lower: String = text
        .to_lowercase()
        .chars()
        .filter(|c| !matches!(c, '\'' | '’'))
        .map(|c| {
            if c.is_alphanumeric() || SYMBOLS.contains(&c) || c == '.' || c == ',' || c == '#' {
                c
            } else {
                ' '
            }
        })
        .collect();
    lower
        .split_whitespace()
        .map(|t| t.trim_matches(|c| c == '.' || c == ',').to_string())
        .filter(|t| !t.is_empty())
        .collect()
}

fn has_digit(t: &str) -> bool {
    t.chars().any(|c| c.is_ascii_digit())
}

fn has_amount(words: &[String]) -> bool {
    words.iter().enumerate().any(|(i, w)| {
        let symbol = w.contains(SYMBOLS);
        if symbol && has_digit(w) {
            return true;
        }
        if symbol && words.get(i + 1).is_some_and(|n| has_digit(n)) {
            return true;
        }
        let number = has_digit(w);
        number
            && (CODES.iter().any(|c| w.ends_with(c))
                || words
                    .get(i + 1)
                    .is_some_and(|n| CODES.contains(&n.as_str()))
                || i.checked_sub(1)
                    .is_some_and(|p| CODES.contains(&words[p].as_str())))
    })
}

fn role_can_commit(role: Role) -> bool {
    matches!(
        role,
        Role::Button | Role::Link | Role::Cell | Role::Text | Role::Image | Role::Other
    )
}

pub fn classify(role: Role, label: &str) -> bool {
    if !role_can_commit(role) {
        return false;
    }
    let words = tokens(label);
    if words.is_empty() || words.len() > MAX_WORDS || words.iter().any(|w| w.starts_with('#')) {
        return false;
    }
    let phrase = words.join(" ");
    if EXACT.contains(&phrase.as_str()) {
        return true;
    }
    if PREFIX
        .iter()
        .any(|p| phrase == *p || phrase.starts_with(&format!("{p} ")))
    {
        return true;
    }
    let delete_account = ["delete account", "delete my account", "delete your account"];
    if matches!(
        words[0].as_str(),
        "delete" | "permanently" | "yes" | "confirm"
    ) && words.len() <= 6
        && delete_account.iter().any(|d| phrase.contains(d))
    {
        return true;
    }
    if words.len() == 1 || !AMOUNT_VERBS.contains(&words[0].as_str()) || !has_amount(&words) {
        return false;
    }
    words[0] != "order"
        || words
            .get(1)
            .is_some_and(|w| has_digit(w) || w.contains(SYMBOLS) || w == "now" || w == "for")
}

fn text_of(e: &Element) -> Option<&str> {
    e.label
        .as_deref()
        .or(e.value.as_deref())
        .map(str::trim)
        .filter(|t| !t.is_empty())
}

/// The smallest labelled element that holds the point.
pub fn hit(now: &Rendered, at: Point) -> Option<&Element> {
    now.elements
        .iter()
        .filter(|e| {
            text_of(e).is_some()
                && e.bounds.w > 0.0
                && at.x >= e.bounds.x
                && at.x <= e.bounds.x + e.bounds.w
                && at.y >= e.bounds.y
                && at.y <= e.bounds.y + e.bounds.h
        })
        .min_by(|a, b| {
            (a.bounds.w * a.bounds.h)
                .partial_cmp(&(b.bounds.w * b.bounds.h))
                .unwrap_or(std::cmp::Ordering::Equal)
        })
}

pub fn guarded(e: &Element) -> bool {
    e.state.contains(ElementState::ENABLED) && text_of(e).is_some_and(|t| classify(e.role, t))
}

/// A guarded button in the field's own form: within three ancestors of the field, never past a list. A field with no
/// ancestor left (the pruned tree keeps no plain containers) shares the screen's top level with the button.
pub fn submit_button<'a>(now: &'a Rendered, field: &Element) -> Option<&'a Element> {
    let search = |starts: Vec<usize>| {
        let mut stack = starts;
        while let Some(i) = stack.pop() {
            if guarded(&now.elements[i]) {
                return Some(&now.elements[i]);
            }
            stack.extend(now.elements[i].children.iter().copied());
        }
        None
    };
    let mut at = now.index_of(field.r#ref)?;
    for _ in 0..3 {
        match now.parents[at] {
            Some(parent) => {
                at = parent;
                if now.elements[at].role == Role::List {
                    return None;
                }
                if let found @ Some(_) = search(vec![at]) {
                    return found;
                }
            }
            None => {
                let roots = (0..now.elements.len())
                    .filter(|&i| now.parents[i].is_none())
                    .collect();
                return search(roots);
            }
        }
    }
    None
}

fn quantity(text: &str) -> bool {
    let digits = text.chars().take_while(char::is_ascii_digit).count();
    let rest = &text[digits..];
    let item = rest
        .trim_start()
        .strip_prefix(['x', '×'])
        .unwrap_or(rest)
        .trim_start();
    (1..=2).contains(&digits)
        && rest.starts_with([' ', 'x', '×'])
        && item.chars().next().is_some_and(char::is_alphabetic)
}

const CONTEXT_WORDS: &[&str] = &[
    "total",
    "subtotal",
    "deliver",
    "address",
    "ship",
    "arriv",
    "qty",
    "quantity",
    "item",
    "recipient",
    "to:",
    "from",
    "pay with",
    "card",
    "tip",
    "fee",
    "tax",
    "balance",
    "account",
    "order",
    "plan",
    "per month",
    "per year",
    "/month",
    "/year",
    "est.",
];

const CONTEXT_LINES: usize = 8;
const CONTEXT_WIDTH: usize = 90;

/// What the screen says about the thing being committed, best effort: visible text with an amount or a word that
/// names an item, a total, a place or a recipient, in screen order, without the button itself.
pub fn context(now: &Rendered, button: &Element) -> Vec<String> {
    let mut lines: Vec<(i32, i32, String)> = Vec::new();
    for e in &now.elements {
        if e.r#ref == button.r#ref
            || !e.children.is_empty()
            || matches!(
                e.role,
                Role::TextField | Role::SearchField | Role::SecureField | Role::Keyboard
            )
            || e.state.contains(ElementState::PASSWORD)
        {
            continue;
        }
        let Some(t) = text_of(e) else { continue };
        if t.chars().count() > 160 {
            continue;
        }
        let words = tokens(t);
        let lower = t.to_lowercase();
        let named = CONTEXT_WORDS.iter().any(|w| lower.contains(w));
        if !(has_amount(&words) || named || quantity(t)) || lines.iter().any(|(_, _, l)| l == t) {
            continue;
        }
        let shown: String = t.chars().take(CONTEXT_WIDTH).collect();
        lines.push((e.bounds.y as i32, e.bounds.x as i32, shown));
    }
    lines.sort();
    lines
        .into_iter()
        .take(CONTEXT_LINES)
        .map(|(_, _, l)| l)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    const POSITIVE: &[(Role, &str)] = &[
        (Role::Button, "Place order · $34.10"),
        (Role::Button, "Place Order"),
        (Role::Button, "Place your order"),
        (Role::Button, "Place your order and pay"),
        (Role::Button, "Buy now"),
        (Role::Button, "Buy Now with 1-Click"),
        (Role::Button, "Buy with Google Pay"),
        (Role::Button, "Buy"),
        (Role::Button, "Pay"),
        (Role::Button, "Pay now"),
        (Role::Button, "Pay $12.00"),
        (Role::Button, "Pay · €8,50"),
        (Role::Button, "Pay 12.00 USD"),
        (Role::Button, "Pay £4.99 now"),
        (Role::Button, "Confirm payment"),
        (Role::Button, "Confirm and pay"),
        (Role::Button, "Checkout and pay"),
        (Role::Button, "Checkout · $34.10"),
        (Role::Button, "Confirm purchase"),
        (Role::Button, "Complete purchase"),
        (Role::Button, "Purchase"),
        (Role::Button, "Order · $18.99"),
        (Role::Button, "Order now"),
        (Role::Button, "Send money"),
        (Role::Button, "Send $20.00"),
        (Role::Button, "Send $20.00 to Sam"),
        (Role::Button, "Transfer"),
        (Role::Button, "Transfer $500.00"),
        (Role::Button, "Confirm transfer"),
        (Role::Button, "Subscribe · $9.99/month"),
        (Role::Button, "Subscribe for $4.99"),
        (Role::Button, "Start subscription"),
        (Role::Button, "Donate $25"),
        (Role::Button, "Delete account"),
        (Role::Button, "Yes, delete my account"),
        (Role::Button, "Permanently delete account"),
        (Role::Link, "Place order"),
        (Role::Other, "Buy now"),
        (Role::Cell, "Place order · $34.10"),
    ];

    const NEGATIVE: &[(Role, &str)] = &[
        (Role::Button, "Payments history"),
        (Role::Button, "Payment methods"),
        (Role::Button, "Pay later"),
        (Role::Button, "Pay bills"),
        (Role::Button, "Send"),
        (Role::Button, "Send message"),
        (Role::Button, "Send feedback"),
        (Role::Button, "Send code"),
        (Role::Button, "Subscribe"),
        (Role::Button, "Subscribed"),
        (Role::Button, "Buy again"),
        (Role::Button, "Add to cart · $12.00"),
        (Role::Button, "Add to basket"),
        (Role::Button, "Checkout"),
        (Role::Button, "Go to checkout"),
        (Role::Button, "View cart"),
        (Role::Button, "Order #4821 · $34.10"),
        (Role::Button, "Order status"),
        (Role::Button, "Order history"),
        (Role::Button, "Your order total is $34.10"),
        (Role::Button, "Purchases"),
        (Role::Button, "Transfers"),
        (Role::Button, "Transfer history"),
        (Role::Button, "Delete"),
        (Role::Button, "Delete message"),
        (Role::Button, "Learn how to delete your account"),
        (Role::Button, "Cancel order"),
        (Role::Button, "Don't buy"),
        (Role::Button, "Cancel"),
        (Role::Button, ""),
        (Role::Tab, "Pay"),
        (Role::Tab, "Transfer"),
        (Role::MenuItem, "Pay"),
        (Role::TextField, "Transfer"),
        (Role::SearchField, "Buy now"),
        (Role::Text, "Total $34.10"),
        (Role::Heading, "Place order"),
    ];

    #[test]
    fn irreversible_labels_are_told_from_look_alikes() {
        let missed: Vec<_> = POSITIVE.iter().filter(|(r, l)| !classify(*r, l)).collect();
        let false_alarms: Vec<_> = NEGATIVE.iter().filter(|(r, l)| classify(*r, l)).collect();
        assert!(missed.is_empty(), "not guarded: {missed:?}");
        assert!(
            false_alarms.is_empty(),
            "guarded by mistake: {false_alarms:?}"
        );
    }

    #[test]
    fn a_guarded_tap_asks_unless_the_human_or_autonomy_said_otherwise() {
        let mut s = Session::default();
        assert_eq!(stance(&mut s, None), Stance::Ask);
        assert_eq!(
            stance(&mut s, Some(Approval::Once)),
            Stance::Run(Some("approved by the human"))
        );
        assert_eq!(stance(&mut s, None), Stance::Ask, "once is not sticky");
        assert_eq!(
            stance(&mut s, Some(Approval::Autonomous)),
            Stance::Run(None)
        );
        assert_eq!(
            stance(&mut s, None),
            Stance::Ask,
            "autonomy is not remembered"
        );
        assert!(matches!(
            stance(&mut s, Some(Approval::Session)),
            Stance::Run(Some(_))
        ));
        assert!(matches!(stance(&mut s, None), Stance::Run(Some(_))));
        assert!(
            !Session::default().guard_allowed,
            "a new session starts closed"
        );
    }
}
