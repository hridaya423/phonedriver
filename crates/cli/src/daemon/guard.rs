//! The approval guard (decision D27): which taps spend money or cannot be undone, and what to tell the human.

mod labels;

use phonedriver_core::device::Gesture;
use phonedriver_core::element::{Actions, Element, ElementState, Role};
use phonedriver_core::error::Category;
use phonedriver_core::geometry::Point;
use phonedriver_core::render::Rendered;

use crate::daemon::session::Session;
use crate::wire::Approval;
use labels::{
    ACCOUNT, ACCOUNT_CJK, AMOUNT_VERBS, DELETE, DELETE_CJK, EXACT, INSTALL, INSTALL_CJK,
    ORDER_VERBS, PAYMENT_CJK, PREFIX, SEND, SEND_CJK,
};

/// What to do with a tap that the classifier guards.
#[derive(Debug, PartialEq, Eq)]
pub enum Stance {
    Run(Option<&'static str>),
    Ask,
}

pub fn stance(s: &mut Session, approval: Option<Approval>, category: Category) -> Stance {
    match approval {
        Some(Approval::Autonomous) => Stance::Run(None),
        Some(Approval::Session) => {
            if !s.guard_allowed.contains(&category) {
                s.guard_allowed.push(category);
            }
            Stance::Run(Some("approved by the human for this session"))
        }
        Some(Approval::Once) => Stance::Run(Some("approved by the human")),
        None if s.guard_allowed.contains(&category) => {
            Stance::Run(Some("approved by the human for this session"))
        }
        None => Stance::Ask,
    }
}

/// Whether a tap of this category is held: payments and account deletion always, the rest when the human asked.
pub fn asks(category: Category, ask: &[Category]) -> bool {
    category.always() || ask.contains(&category)
}

const OTHER_CATEGORIES: [(Category, &[&str], &[&str]); 4] = [
    (Category::Account, ACCOUNT, ACCOUNT_CJK),
    (Category::Send, SEND, SEND_CJK),
    (Category::Delete, DELETE, DELETE_CJK),
    (Category::Install, INSTALL, INSTALL_CJK),
];

const CODES: &[&str] = &[
    "usd", "eur", "gbp", "cad", "aud", "inr", "jpy", "mxn", "chf", "cny", "brl", "krw", "sek",
    "nok", "dkk", "kr", "pln", "zł", "zl", "aed", "sar", "qar", "kwd", "rub", "zar", "hkd", "sgd",
    "nzd", "thb", "php", "idr", "rp", "myr", "vnd", "ils", "czk", "kč", "huf", "ron", "lei", "bgn",
    "ngn", "egp", "kes", "cop", "ars", "clp", "uah", "twd",
];

const SYMBOLS: &[char] = &[
    '$', '€', '£', '¥', '￥', '₹', '₩', '₺', '₽', '฿', '₱', '₫', '₪', '₦', '₴', '₸', '₡', '₲', '₵',
];

pub fn app_name(now: &Rendered) -> Option<String> {
    now.header
        .app
        .as_ref()
        .map(|a| a.name.clone().unwrap_or_else(|| a.id.clone()))
}

const MAX_WORDS: usize = 9;

pub(crate) fn tokens(text: &str) -> Vec<String> {
    let lower: String = phonedriver_core::text::fold(text)
        .chars()
        .filter(|c| *c != '\'')
        .map(|c| {
            if c.is_alphanumeric()
                || SYMBOLS.contains(&c)
                || matches!(c, '.' | ',' | '#')
                || is_devanagari(c)
            {
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

fn is_devanagari(c: char) -> bool {
    ('\u{0900}'..='\u{097F}').contains(&c) && !matches!(c, '।' | '॥')
}

pub(crate) fn is_cjk(c: char) -> bool {
    matches!(
        c,
        '\u{1100}'..='\u{11FF}'
            | '\u{3040}'..='\u{30FF}'
            | '\u{3400}'..='\u{4DBF}'
            | '\u{4E00}'..='\u{9FFF}'
            | '\u{AC00}'..='\u{D7AF}'
            | '々'
    )
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
        if symbol && i > 0 && has_digit(&words[i - 1]) {
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
        Role::Button
            | Role::Link
            | Role::Cell
            | Role::Text
            | Role::Image
            | Role::Other
            | Role::Slider
    )
}

/// Japanese, Chinese and Korean write no spaces between words, so the label is read as one run: the characters up to
/// the first space, digit or symbol must be a known phrase (`购买` yes, `购买记录` no), and what follows must be
/// an amount or nothing.
fn classify_cjk(label: &str) -> Option<Category> {
    let run: String = label.split_whitespace().collect();
    if run.chars().count() > 40 || run.contains('#') {
        return None;
    }
    let head: String = run.chars().take_while(|c| is_cjk(*c)).collect();
    let tail = &run[head.len()..];
    let plain = tail.chars().all(|c| !c.is_alphanumeric())
        || tail.chars().any(|c| c.is_numeric() || SYMBOLS.contains(&c));
    if !plain {
        return None;
    }
    if PAYMENT_CJK.contains(&head.as_str()) {
        return Some(Category::Payment);
    }
    OTHER_CATEGORIES
        .iter()
        .find(|(_, _, cjk)| cjk.contains(&head.as_str()))
        .map(|(c, _, _)| *c)
}

fn is_payment(words: &[String], phrase: &str) -> bool {
    if EXACT.contains(&phrase) {
        return true;
    }
    if PREFIX
        .iter()
        .any(|p| phrase == *p || phrase.starts_with(&format!("{p} ")))
    {
        return true;
    }
    if words.len() == 2
        && words[0] == "pay"
        && words[1]
            .chars()
            .all(|c| c.is_ascii_digit() || matches!(c, '.' | ','))
    {
        return true;
    }
    if words.len() == 1 || !has_amount(words) {
        return false;
    }
    let verb = words[0].as_str();
    if ORDER_VERBS.contains(&verb) {
        return words
            .get(1)
            .is_some_and(|w| has_digit(w) || w.contains(SYMBOLS) || w == "now" || w == "for");
    }
    AMOUNT_VERBS.contains(&verb)
}

pub fn classify(role: Role, label: &str) -> Option<Category> {
    if !role_can_commit(role) {
        return None;
    }
    let label = &phonedriver_core::text::fold(label);
    if label.trim_start().starts_with(is_cjk) {
        return classify_cjk(label);
    }
    let words = tokens(label);
    if words.is_empty() || words.len() > MAX_WORDS || words.iter().any(|w| w.starts_with('#')) {
        return None;
    }
    let phrase = words.join(" ");
    if is_payment(&words, &phrase) {
        return Some(Category::Payment);
    }
    let delete_account = ["delete account", "delete my account", "delete your account"];
    if matches!(
        words[0].as_str(),
        "delete" | "permanently" | "yes" | "confirm"
    ) && words.len() <= 6
        && delete_account.iter().any(|d| phrase.contains(d))
    {
        return Some(Category::Account);
    }
    OTHER_CATEGORIES
        .iter()
        .find(|(_, exact, _)| exact.contains(&phrase.as_str()))
        .map(|(c, _, _)| *c)
}

fn text_of(e: &Element) -> Option<&str> {
    e.label
        .as_deref()
        .or(e.value.as_deref())
        .map(str::trim)
        .filter(|t| !t.is_empty())
}

fn holds(r: &phonedriver_core::geometry::Rect, at: Point) -> bool {
    r.w > 0.0 && at.x >= r.x && at.x <= r.x + r.w && at.y >= r.y && at.y <= r.y + r.h
}

/// The smallest labelled element that holds the point.
pub fn hit(now: &Rendered, at: Point) -> Option<&Element> {
    now.elements
        .iter()
        .filter(|e| text_of(e).is_some() && holds(&e.bounds, at))
        .min_by(|a, b| {
            (a.bounds.w * a.bounds.h)
                .partial_cmp(&(b.bounds.w * b.bounds.h))
                .unwrap_or(std::cmp::Ordering::Equal)
        })
}

fn takes_touch(e: &Element) -> bool {
    e.actions.intersects(Actions::TAP | Actions::EDIT)
        || matches!(e.role, Role::Button | Role::Link)
        || e.role.is_text_entry()
}

/// The guarded element a touch on `e` commits: `e` itself, or, when `e` takes no touch of its own (a price inside a
/// "Place order" row), the nearest ancestor that does.
pub fn receiver<'a>(now: &'a Rendered, e: &'a Element, ask: &[Category]) -> Option<&'a Element> {
    let mut at = now.index_of(e.r#ref);
    let mut current = e;
    loop {
        if guarded(current, ask).is_some() {
            return Some(current);
        }
        if takes_touch(current) {
            return None;
        }
        let parent = at.and_then(|i| now.parents[i])?;
        at = Some(parent);
        current = &now.elements[parent];
    }
}

/// The guarded element a touch at the point commits.
pub fn clicked<'a>(now: &'a Rendered, at: Point, ask: &[Category]) -> Option<&'a Element> {
    receiver(now, hit(now, at)?, ask)
}

const SLIDE_WORDS: &[&str] = &[
    "slide",
    "swipe",
    "desliz",
    "arrast",
    "gliss",
    "wisch",
    "schieb",
    "scorri",
    "veeg",
    "schuif",
    "स्लाइड",
    "スライド",
    "スワイプ",
    "滑动",
    "右滑",
    "밀어서",
    "슬라이드",
];

/// A control that commits when a finger moves along it: a slider, or a label such as "Slide to pay".
fn slides(e: &Element) -> bool {
    e.role == Role::Slider
        || text_of(e).is_some_and(|t| {
            let lower = t.to_lowercase();
            SLIDE_WORDS.iter().any(|w| lower.contains(w))
        })
}

/// Movement within which a finger still clicks what it went down on, in points (Android's touch slop is 8 dp).
const TOUCH_SLOP: f32 = 8.0;

/// Whether a finger of the gesture stays where it went down, as a tap does.
pub fn tap_like(g: &Gesture) -> bool {
    g.fingers.iter().any(|f| {
        f.first().is_some_and(|start| {
            f.iter()
                .all(|p| (p.at.x - start.at.x).hypot(p.at.y - start.at.y) < TOUCH_SLOP)
        })
    })
}

/// Whether a gesture whose first finger went down on `e` commits it: it lifts inside `e`, or `e` is a "Slide to pay"
/// control. For a start found without a screen to look up its ancestors.
pub fn strokes(e: &Element, g: &Gesture, ask: &[Category]) -> bool {
    guarded(e, ask).is_some()
        && g.fingers
            .first()
            .and_then(|f| f.last())
            .is_some_and(|last| holds(&e.bounds, last.at) || slides(e))
}

/// The guarded element a gesture commits. A finger that goes down on a guarded element and lifts inside it is a
/// click there however it moved (a zero-length swipe, a one-pixel drag); one that goes down on a "Slide to pay"
/// control commits it wherever it lifts. With `drop`, where a finger lifts counts too (dragging an item onto
/// "Delete"). Payments win over the other categories.
pub fn touched<'a>(
    now: &'a Rendered,
    g: &Gesture,
    drop: bool,
    ask: &[Category],
) -> Option<&'a Element> {
    let mut found: Option<(&Element, Category)> = None;
    for finger in &g.fingers {
        let (Some(first), Some(last)) = (finger.first(), finger.last()) else {
            continue;
        };
        let down = clicked(now, first.at, ask).filter(|e| holds(&e.bounds, last.at) || slides(e));
        let up = drop.then(|| clicked(now, last.at, ask)).flatten();
        for e in [down, up].into_iter().flatten() {
            if let Some(c) = guarded(e, ask)
                && found.is_none_or(|(_, best)| c < best)
            {
                found = Some((e, c));
            }
        }
    }
    found.map(|(e, _)| e)
}

/// What `e` would commit when the human wants to be asked about it, whether or not it is enabled yet.
pub fn category(e: &Element, ask: &[Category]) -> Option<Category> {
    text_of(e)
        .and_then(|t| classify(e.role, t))
        .filter(|c| asks(*c, ask))
}

/// What tapping `e` would commit, when the human wants to be asked about it.
pub fn guarded(e: &Element, ask: &[Category]) -> Option<Category> {
    if !e.state.contains(ElementState::ENABLED) {
        return None;
    }
    category(e, ask)
}

/// The guarded button that enter would send: the focused element itself when it is such a button, else the focused
/// field's form (`submit_button`). With nothing focused and the keyboard up, the whole screen counts.
pub fn entered<'a>(now: &'a Rendered, ask: &[Category]) -> Option<&'a Element> {
    let focused = now
        .elements
        .iter()
        .find(|e| e.state.contains(ElementState::FOCUSED));
    match focused {
        Some(f) if !f.role.is_text_entry() => receiver(now, f, ask),
        Some(f) => submit_button(now, Some(f), ask),
        None if now.header.keyboard.is_some() => submit_button(now, None, ask),
        None => None,
    }
}

/// A guarded button in the field's own form: within three ancestors of the field, never past a list. A field with no
/// ancestor left (the pruned tree keeps no plain containers), or no known field, shares the screen's top level with
/// the button. A disabled button counts: a form can enable its Pay button only once the field is filled, between
/// this read and the enter. When the form holds several, the one that matters most (payments first) is returned.
pub fn submit_button<'a>(
    now: &'a Rendered,
    field: Option<&Element>,
    ask: &[Category],
) -> Option<&'a Element> {
    let search = |starts: Vec<usize>| {
        let mut stack = starts;
        let mut found: Option<(&Element, Category)> = None;
        while let Some(i) = stack.pop() {
            if let Some(c) = category(&now.elements[i], ask)
                && found.is_none_or(|(_, best)| c < best)
            {
                found = Some((&now.elements[i], c));
            }
            stack.extend(now.elements[i].children.iter().copied());
        }
        found.map(|(e, _)| e)
    };
    let roots = || {
        (0..now.elements.len())
            .filter(|&i| now.parents[i].is_none())
            .collect()
    };
    let Some(mut at) = field.and_then(|f| now.index_of(f.r#ref)) else {
        return search(roots());
    };
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
            None => return search(roots()),
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
        (Role::Button, "Pay with Google Pay"),
        (Role::Button, "Pay 50 kr"),
        (Role::Button, "Pay 500"),
        (Role::Button, "Pay 120 zł"),
        (Role::Button, "Pay 99 AED"),
        (Role::Button, "Buy for ₺250"),
        (Role::Other, "Slide to pay"),
        (Role::Other, "Swipe to confirm order"),
        (Role::Other, "Slide to pay $34.10"),
        (Role::Other, "Desliza para pagar"),
        (Role::Other, "Glissez pour payer"),
        (Role::Other, "Zum Bezahlen wischen"),
        (Role::Other, "Scorri per pagare"),
        (Role::Other, "Veeg om te betalen"),
        (Role::Other, "Deslize para pagar"),
        (Role::Other, "भुगतान करने के लिए स्लाइड करें"),
        (Role::Other, "滑动支付"),
        (Role::Other, "スライドして支払う"),
        (Role::Other, "밀어서 결제"),
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

    fn by_default(role: Role, label: &str) -> bool {
        classify(role, label).is_some_and(|c| asks(c, &[]))
    }

    #[test]
    fn a_label_in_another_code_point_form_is_the_same_label() {
        for label in [
            "\u{FF30}\u{FF41}\u{FF59} \u{FF4E}\u{FF4F}\u{FF57}",
            "Pay\u{200B} now",
            "P\u{00AD}ay now",
            "Place\u{00A0}order",
            "Buy\u{2060} now",
            "\u{202E}Pay now",
            "Pl\u{0430}ce order",
        ] {
            let got = classify(Role::Button, label);
            if label.contains('\u{0430}') {
                assert_eq!(got, None, "a Cyrillic look-alike is not folded: {label:?}");
            } else {
                assert_eq!(got, Some(Category::Payment), "{label:?}");
            }
        }
        assert_eq!(classify(Role::Button, "Don\u{2019}t buy"), None);
        assert_eq!(
            classify(Role::Button, "購入する ￥１，２００"),
            classify(Role::Button, "購入する ¥1,200")
        );
    }

    #[test]
    fn irreversible_labels_are_told_from_look_alikes() {
        let missed: Vec<_> = POSITIVE
            .iter()
            .filter(|(r, l)| !by_default(*r, l))
            .collect();
        let false_alarms: Vec<_> = NEGATIVE.iter().filter(|(r, l)| by_default(*r, l)).collect();
        assert!(missed.is_empty(), "not guarded: {missed:?}");
        assert!(
            false_alarms.is_empty(),
            "guarded by mistake: {false_alarms:?}"
        );
    }

    const LANGUAGES: &[(&str, &[&str], &[&str])] = &[
        (
            "es",
            &[
                "Pagar",
                "Pagar ahora",
                "Comprar",
                "Comprar ahora",
                "Realizar pedido",
                "Realizar el pedido",
                "Confirmar pedido",
                "Confirmar y pagar",
                "Pagar 34,10 €",
                "Pagar · $34.10",
                "Realizar pedido · 34,10 €",
                "Enviar dinero",
                "Transferir",
                "Transferir 500,00 €",
                "Eliminar cuenta",
                "Eliminar mi cuenta",
                "Sí, eliminar mi cuenta",
            ],
            &[
                "Métodos de pago",
                "Historial de pedidos",
                "Mis pedidos",
                "Mis compras",
                "Pagar más tarde",
                "Añadir al carrito",
                "Comprar de nuevo",
                "Cancelar pedido",
                "Enviar",
                "Enviar mensaje",
                "Suscribirse",
                "Pedido #4821 · 34,10 €",
                "Pagos",
                "Transferencias",
                "Eliminar mensaje",
                "Cómo eliminar mi cuenta",
            ],
        ),
        (
            "pt",
            &[
                "Pagar",
                "Pagar agora",
                "Comprar agora",
                "Fazer pedido",
                "Finalizar pedido",
                "Confirmar pagamento",
                "Confirmar pedido",
                "Pagar R$ 34,10",
                "Comprar · R$ 12,00",
                "Enviar dinheiro",
                "Transferir",
                "Excluir conta",
                "Excluir minha conta",
            ],
            &[
                "Formas de pagamento",
                "Meus pedidos",
                "Histórico de compras",
                "Adicionar ao carrinho",
                "Comprar novamente",
                "Cancelar pedido",
                "Enviar mensagem",
                "Assinar",
                "Pagar depois",
                "Pedido #123 · R$ 12,00",
                "Como excluir minha conta",
            ],
        ),
        (
            "fr",
            &[
                "Payer",
                "Payer maintenant",
                "Acheter",
                "Acheter maintenant",
                "Commander",
                "Passer la commande",
                "Confirmer la commande",
                "Valider la commande",
                "Confirmer et payer",
                "Payer 34,10 €",
                "Payer · 12,00 €",
                "Commander · 34,10 €",
                "Envoyer de l'argent",
                "Envoyer de l’argent",
                "Supprimer mon compte",
                "Supprimer le compte",
                "Oui, supprimer mon compte",
            ],
            &[
                "Moyens de paiement",
                "Mes commandes",
                "Historique des commandes",
                "Ajouter au panier",
                "Acheter à nouveau",
                "Annuler la commande",
                "Payer plus tard",
                "Envoyer",
                "Envoyer un message",
                "Commande #4821 · 34,10 €",
                "Suivre ma commande",
                "S'abonner",
                "Comment supprimer mon compte",
            ],
        ),
        (
            "de",
            &[
                "Bezahlen",
                "Jetzt bezahlen",
                "Jetzt kaufen",
                "Jetzt kaufen mit Google Pay",
                "Kaufen",
                "Bestellen",
                "Zahlungspflichtig bestellen",
                "Kostenpflichtig bestellen",
                "Jetzt bestellen",
                "Bestellung aufgeben",
                "Bestellung abschicken",
                "Kauf abschließen",
                "Bezahlen · 34,10 €",
                "Geld senden",
                "Überweisen",
                "Konto löschen",
                "Mein Konto löschen",
                "Ja, Konto löschen",
            ],
            &[
                "Zahlungsarten",
                "Zahlungsmethoden",
                "Bestellungen",
                "Meine Bestellungen",
                "Bestellverlauf",
                "In den Warenkorb",
                "Erneut kaufen",
                "Bestellung stornieren",
                "Später bezahlen",
                "Senden",
                "Nachricht senden",
                "Abonnieren",
                "Bestellung #4821 · 34,10 €",
                "Zur Kasse",
                "Wie kann ich mein Konto löschen",
            ],
        ),
        (
            "it",
            &[
                "Paga",
                "Paga ora",
                "Pagare",
                "Acquista",
                "Acquista ora",
                "Compra ora",
                "Conferma ordine",
                "Conferma e paga",
                "Conferma pagamento",
                "Completa ordine",
                "Effettua l'ordine",
                "Ordina ora",
                "Paga 34,10 €",
                "Acquista · 12,00 €",
                "Invia denaro",
                "Trasferisci",
                "Elimina account",
                "Elimina il mio account",
            ],
            &[
                "Metodi di pagamento",
                "I miei ordini",
                "Cronologia ordini",
                "Aggiungi al carrello",
                "Acquista di nuovo",
                "Annulla ordine",
                "Paga dopo",
                "Invia messaggio",
                "Ordina per",
                "Ordine #4821 · 34,10 €",
                "Acquisti",
                "Come eliminare il mio account",
            ],
        ),
        (
            "nl",
            &[
                "Betalen",
                "Nu betalen",
                "Betaal nu",
                "Kopen",
                "Nu kopen",
                "Koop nu",
                "Bestellen",
                "Bestelling plaatsen",
                "Bestelling bevestigen",
                "Betaal € 34,10",
                "Bestellen en betalen",
                "Geld versturen",
                "Overmaken",
                "Account verwijderen",
                "Mijn account verwijderen",
            ],
            &[
                "Betaalmethoden",
                "Mijn bestellingen",
                "Bestelgeschiedenis",
                "Toevoegen aan winkelwagen",
                "Opnieuw kopen",
                "Bestelling annuleren",
                "Later betalen",
                "Verstuur bericht",
                "Bestelling #4821 · € 34,10",
                "Aankopen",
                "Betalingen",
                "Hoe verwijder ik mijn account",
            ],
        ),
        (
            "ja",
            &[
                "注文を確定する",
                "注文を確定",
                "購入",
                "購入する",
                "今すぐ購入",
                "今すぐ買う",
                "支払う",
                "支払いを確定する",
                "購入する（¥1,200）",
                "購入 ¥1,200",
                "送金する",
                "アカウントを削除",
                "アカウントを削除する",
                "退会する",
            ],
            &[
                "支払い方法",
                "お支払い方法",
                "購入履歴",
                "注文履歴",
                "カートに入れる",
                "カート",
                "購入手続きへ",
                "購入済み",
                "購入完了",
                "メッセージを削除",
                "アカウント設定",
                "支払い履歴",
            ],
        ),
        (
            "zh",
            &[
                "购买",
                "立即购买",
                "立即支付",
                "确认支付",
                "确认付款",
                "提交订单",
                "立即下单",
                "支付",
                "支付 ¥99.00",
                "立即支付¥99",
                "确认转账",
                "注销账号",
                "删除账号",
                "立即开通",
                "确认协议并支付",
            ],
            &[
                "支付方式",
                "付款方式",
                "我的订单",
                "订单详情",
                "购买记录",
                "购买须知",
                "去结算",
                "结算",
                "加入购物车",
                "购物车",
                "已购买",
                "支付成功",
                "取消订单",
                "删除消息",
                "支付设置",
            ],
        ),
        (
            "ko",
            &[
                "결제하기",
                "주문하기",
                "구매하기",
                "구매",
                "바로 구매",
                "바로구매",
                "송금하기",
                "이체하기",
                "회원탈퇴",
                "계정 삭제",
                "결제하기 ₩12,000",
                "구매하기 · ₩12,000",
            ],
            &[
                "결제 수단",
                "결제수단",
                "결제 내역",
                "주문 내역",
                "주문 완료",
                "결제 완료",
                "장바구니",
                "장바구니 담기",
                "구매 내역",
                "메시지 삭제",
                "계정 설정",
            ],
        ),
        (
            "hi",
            &[
                "भुगतान करें",
                "अभी भुगतान करें",
                "भुगतान करें ₹499",
                "खरीदें",
                "अभी खरीदें",
                "ऑर्डर करें",
                "अभी ऑर्डर करें",
                "ऑर्डर कन्फर्म करें",
                "पैसे भेजें",
                "खाता हटाएं",
                "अकाउंट डिलीट करें",
            ],
            &[
                "भुगतान विधि",
                "भुगतान इतिहास",
                "मेरे ऑर्डर",
                "ऑर्डर इतिहास",
                "कार्ट में जोड़ें",
                "भुगतान जारी रखें",
                "ऑर्डर रद्द करें",
                "संदेश भेजें",
                "भुगतान राशि ₹500",
                "भुगतान के तरीके",
            ],
        ),
    ];

    #[test]
    fn labels_in_other_languages_are_told_from_look_alikes() {
        let mut wrong = Vec::new();
        for (lang, positive, negative) in LANGUAGES {
            for label in *positive {
                if !by_default(Role::Button, label) {
                    wrong.push(format!("{lang}: not guarded: {label}"));
                }
            }
            for label in *negative {
                if by_default(Role::Button, label) {
                    wrong.push(format!("{lang}: guarded by mistake: {label}"));
                }
            }
        }
        assert!(wrong.is_empty(), "{wrong:#?}");
    }

    #[test]
    fn a_label_in_another_language_on_a_navigating_role_is_not_guarded() {
        for label in ["Pagar", "立即支付", "भुगतान करें", "Jetzt kaufen"] {
            for role in [Role::Tab, Role::MenuItem, Role::TextField, Role::Heading] {
                assert_eq!(classify(role, label), None, "{role:?} {label}");
            }
        }
    }

    const OPT_IN: &[(Category, &[&str], &[&str])] = &[
        (
            Category::Send,
            &[
                "Send",
                "Send message",
                "Send email",
                "Send now",
                "Reply",
                "Reply all",
                "Post",
                "Post reply",
                "Publish",
                "Tweet",
                "Share",
                "Forward",
                "Enviar",
                "Responder",
                "Publicar",
                "Compartir",
                "Envoyer",
                "Répondre",
                "Partager",
                "Senden",
                "Antworten",
                "Teilen",
                "Veröffentlichen",
                "Invia",
                "Rispondi",
                "Condividi",
                "Verzenden",
                "Beantwoorden",
                "Delen",
                "送信",
                "返信する",
                "投稿",
                "共有",
                "发送",
                "回复",
                "发布",
                "分享",
                "보내기",
                "답장",
                "게시",
                "공유",
                "भेजें",
                "जवाब दें",
                "पोस्ट करें",
                "साझा करें",
            ],
            &[
                "Sent",
                "Sent items",
                "Replies",
                "Posts",
                "Shared with me",
                "Post office",
                "Share settings",
                "Message",
                "送信済み",
                "已发送",
                "共有設定",
            ],
        ),
        (
            Category::Delete,
            &[
                "Delete",
                "Delete message",
                "Delete all",
                "Delete forever",
                "Delete photo",
                "Remove",
                "Remove all",
                "Erase",
                "Erase all data",
                "Clear all",
                "Clear history",
                "Discard changes",
                "Eliminar",
                "Borrar",
                "Quitar",
                "Supprimer",
                "Effacer",
                "Löschen",
                "Entfernen",
                "Elimina",
                "Rimuovi",
                "Verwijderen",
                "Wissen",
                "削除",
                "削除する",
                "消去",
                "删除",
                "清空",
                "삭제",
                "지우기",
                "हटाएं",
                "डिलीट करें",
            ],
            &[
                "Deleted",
                "Deleted items",
                "Recently deleted",
                "Removed",
                "Clearance",
                "已删除",
                "削除履歴",
            ],
        ),
        (
            Category::Install,
            &[
                "Install",
                "Uninstall",
                "Install app",
                "Instalar",
                "Desinstalar",
                "Installer",
                "Désinstaller",
                "Installieren",
                "Deinstallieren",
                "Installa",
                "Disinstalla",
                "Installeren",
                "インストール",
                "アンインストール",
                "安装",
                "卸载",
                "설치",
                "इंस्टॉल करें",
                "अनइंस्टॉल करें",
            ],
            &[
                "Installed",
                "Installing",
                "Installation guide",
                "Install date",
            ],
        ),
    ];

    #[test]
    fn opt_in_labels_are_sorted_into_their_category_and_told_from_look_alikes() {
        let mut wrong = Vec::new();
        for (category, positive, negative) in OPT_IN {
            for label in *positive {
                let got = classify(Role::Button, label);
                if got != Some(*category) {
                    wrong.push(format!("{label}: {got:?}, wanted {category:?}"));
                }
            }
            for label in *negative {
                let got = classify(Role::Button, label);
                if got.is_some() {
                    wrong.push(format!("{label}: {got:?}, wanted nothing"));
                }
            }
        }
        assert!(wrong.is_empty(), "{wrong:#?}");
    }

    #[test]
    fn money_and_accounts_win_over_the_opt_in_words() {
        let table = [
            ("Send $20.00", Category::Payment),
            ("Send money", Category::Payment),
            ("Delete account", Category::Account),
            ("Konto löschen", Category::Account),
            ("アカウントを削除", Category::Account),
            ("Place order · $34.10", Category::Payment),
        ];
        for (label, want) in table {
            assert_eq!(classify(Role::Button, label), Some(want), "{label}");
        }
    }

    #[test]
    fn payments_and_account_deletion_are_always_asked_and_the_rest_only_on_request() {
        for always in [Category::Payment, Category::Account] {
            assert!(asks(always, &[]), "{always:?}");
        }
        for opt in [Category::Send, Category::Delete, Category::Install] {
            assert!(!asks(opt, &[]), "{opt:?} on by default");
            assert!(asks(opt, &[opt]), "{opt:?} not asked when turned on");
            for other in [Category::Send, Category::Delete, Category::Install] {
                assert_eq!(asks(opt, &[other]), opt == other, "{opt:?} with {other:?}");
            }
        }
    }

    #[test]
    fn a_guarded_tap_asks_unless_the_human_or_autonomy_said_otherwise() {
        let pay = Category::Payment;
        let mut s = Session::default();
        assert_eq!(stance(&mut s, None, pay), Stance::Ask);
        assert_eq!(
            stance(&mut s, Some(Approval::Once), pay),
            Stance::Run(Some("approved by the human"))
        );
        assert_eq!(stance(&mut s, None, pay), Stance::Ask, "once is not sticky");
        assert_eq!(
            stance(&mut s, Some(Approval::Autonomous), pay),
            Stance::Run(None)
        );
        assert_eq!(
            stance(&mut s, None, pay),
            Stance::Ask,
            "autonomy is not remembered"
        );
        assert!(matches!(
            stance(&mut s, Some(Approval::Session), pay),
            Stance::Run(Some(_))
        ));
        assert!(matches!(stance(&mut s, None, pay), Stance::Run(Some(_))));
        assert_eq!(
            stance(&mut s, None, Category::Send),
            Stance::Ask,
            "a session approval covers its own category only"
        );
        assert!(
            Session::default().guard_allowed.is_empty(),
            "a new session starts closed"
        );
    }

    use phonedriver_core::device::TimedPoint;
    use phonedriver_core::geometry::Scale;
    use phonedriver_core::raw::RawTree;

    fn screen(nodes: &str, keyboard: bool) -> Rendered {
        let ime = if keyboard {
            r#",{"id":2,"type":"input_method","bounds":[0,600,360,800]}"#
        } else {
            ""
        };
        let json = format!(
            r#"{{"windows":[{{"id":1,"type":"application","package":"com.example.shop","bounds":[0,0,360,800]}}{ime}],"nodes":[{nodes}]}}"#
        );
        let tree = RawTree::from_android_json(&json, Scale { density: 1.0 }).unwrap();
        let mut refs = phonedriver_core::refs::RefTable::default();
        let nodes = phonedriver_core::prune::prune(&tree);
        let assigned = refs.assign(&tree, &nodes);
        let mut header = phonedriver_core::render::Header::default();
        if keyboard {
            header.keyboard = Some(phonedriver_core::geometry::Rect {
                x: 0.0,
                y: 600.0,
                w: 360.0,
                h: 200.0,
            });
        }
        Rendered::build(nodes, &assigned, header)
    }

    const ROOT: &str =
        r#"{"p":-1,"w":1,"cls":"android.widget.FrameLayout","b":[0,0,360,800],"f":"vVe"}"#;

    fn finger(points: &[(f32, f32, u32)]) -> Vec<TimedPoint> {
        points
            .iter()
            .map(|&(x, y, t_ms)| TimedPoint {
                at: Point { x, y },
                t_ms,
            })
            .collect()
    }

    fn one(points: &[(f32, f32, u32)]) -> Gesture {
        Gesture {
            fingers: vec![finger(points)],
        }
    }

    fn label(e: Option<&Element>) -> Option<String> {
        e.and_then(|e| e.label.clone())
    }

    #[test]
    fn a_finger_that_goes_down_and_lifts_on_a_guarded_button_is_a_click_however_it_moved() {
        let now = screen(
            &format!(
                r#"{ROOT},{{"p":0,"w":1,"cls":"android.widget.Button","text":"Place order","b":[0,400,360,450],"f":"vVe","a":["tap"]}}"#
            ),
            false,
        );
        let zero_swipe = one(&[(180.0, 425.0, 0), (180.0, 425.0, 90)]);
        assert_eq!(
            label(touched(&now, &zero_swipe, false, &[])).as_deref(),
            Some("Place order")
        );
        assert!(tap_like(&zero_swipe));
        assert!(!tap_like(&one(&[(180.0, 425.0, 0), (180.0, 300.0, 90)])));
        let nudge = one(&[(180.0, 425.0, 0), (181.0, 425.0, 60)]);
        assert_eq!(
            label(touched(&now, &nudge, false, &[])).as_deref(),
            Some("Place order")
        );
        let away = one(&[(180.0, 425.0, 0), (180.0, 100.0, 300)]);
        assert_eq!(
            touched(&now, &away, false, &[]),
            None,
            "a stroke that leaves the button is a scroll"
        );
        let onto = one(&[(180.0, 100.0, 0), (180.0, 425.0, 900)]);
        assert_eq!(touched(&now, &onto, false, &[]), None);
        assert_eq!(
            label(touched(&now, &onto, true, &[])).as_deref(),
            Some("Place order"),
            "a drop onto it"
        );
    }

    #[test]
    fn a_stroke_along_a_slide_to_pay_track_is_guarded() {
        let now = screen(
            &format!(
                r#"{ROOT},{{"p":0,"w":1,"cls":"android.widget.FrameLayout","text":"Slide to pay $34.10","b":[20,600,340,660],"f":"vVe"}},{{"p":1,"w":1,"cls":"android.widget.ImageView","b":[20,600,80,660],"f":"vVe"}}"#
            ),
            false,
        );
        let slide = one(&[(50.0, 630.0, 0), (330.0, 630.0, 400)]);
        assert_eq!(
            label(touched(&now, &slide, false, &[])).as_deref(),
            Some("Slide to pay $34.10")
        );
        let past_the_end = one(&[(50.0, 630.0, 0), (360.0, 630.0, 400)]);
        assert_eq!(
            label(touched(&now, &past_the_end, false, &[])).as_deref(),
            Some("Slide to pay $34.10")
        );
    }

    #[test]
    fn a_price_inside_a_place_order_row_commits_the_row() {
        let now = screen(
            &format!(
                r#"{ROOT},{{"p":0,"w":1,"cls":"android.widget.LinearLayout","text":"Place order","b":[0,400,360,450],"f":"vVe","a":["tap"]}},{{"p":1,"w":1,"cls":"android.widget.TextView","text":"$34.10","b":[280,410,350,440],"f":"vVe"}},{ROW_BACK}"#,
                ROW_BACK = r#"{"p":0,"w":1,"cls":"android.widget.LinearLayout","text":"Pay now","b":[0,500,360,550],"f":"vVe","a":["tap"]},{"p":3,"w":1,"cls":"android.widget.Button","text":"Edit","b":[280,510,350,540],"f":"vVe","a":["tap"]}"#
            ),
            false,
        );
        assert_eq!(
            label(clicked(&now, Point { x: 300.0, y: 425.0 }, &[])).as_deref(),
            Some("Place order")
        );
        assert_eq!(
            clicked(&now, Point { x: 300.0, y: 525.0 }, &[]),
            None,
            "a button of its own inside the row takes the touch"
        );
    }

    #[test]
    fn enter_sends_the_focused_fields_form_even_while_its_pay_button_is_disabled() {
        let form = |focus: &str| {
            screen(
                &format!(
                    r#"{ROOT},{{"p":0,"w":1,"cls":"android.widget.EditText","text":"","b":[0,300,360,350],"f":"vVe{focus}","a":["tap","set_text"]}},{{"p":0,"w":1,"cls":"android.widget.Button","text":"Pay €250.00","b":[0,400,360,450],"f":"vV","a":["tap"]}}"#
                ),
                !focus.is_empty(),
            )
        };
        let typing = form("EF");
        assert_eq!(label(entered(&typing, &[])).as_deref(), Some("Pay €250.00"));
        let field = typing
            .elements
            .iter()
            .find(|e| e.role.is_text_entry())
            .unwrap();
        assert_eq!(
            label(submit_button(&typing, Some(field), &[])).as_deref(),
            Some("Pay €250.00")
        );
        assert_eq!(
            label(submit_button(&typing, None, &[])).as_deref(),
            Some("Pay €250.00")
        );
        assert_eq!(
            entered(&form(""), &[]),
            None,
            "enter with nothing focused and no keyboard sends nothing"
        );
    }

    #[test]
    fn enter_on_a_focused_pay_button_clicks_it() {
        let now = screen(
            &format!(
                r#"{ROOT},{{"p":0,"w":1,"cls":"android.widget.Button","text":"Pay €49.99","b":[0,400,360,450],"f":"vVeF","a":["tap"]}}"#
            ),
            false,
        );
        assert_eq!(label(entered(&now, &[])).as_deref(), Some("Pay €49.99"));
    }
}
