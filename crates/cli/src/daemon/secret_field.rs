//! Fields the model must not fill with plain text (decision D30): passwords, one-time codes, PINs and card security
//! codes.

use phonedriver_core::element::{Element, ElementState, Role};
use phonedriver_core::error::SecretKind;

use crate::daemon::guard::{is_cjk, tokens};

const CODE_PHRASES: &[&str] = &[
    "otp",
    "one time password",
    "one time code",
    "one time passcode",
    "verification code",
    "verify code",
    "sms code",
    "login code",
    "sign in code",
    "confirmation code",
    "authentication code",
    "authenticator code",
    "auth code",
    "2fa",
    "two factor",
    "código de verificación",
    "código de verificação",
    "código de confirmación",
    "código de confirmação",
    "code de vérification",
    "code de confirmation",
    "bestätigungscode",
    "verifizierungscode",
    "verifikationscode",
    "einmalpasswort",
    "einmalcode",
    "codice di verifica",
    "codice di conferma",
    "verificatiecode",
    "bevestigingscode",
    "eenmalige code",
    "ओटीपी",
    "सत्यापन कोड",
    "वन टाइम पासवर्ड",
    "验证码",
    "驗證碼",
    "校验码",
    "动态密码",
    "一次性密码",
    "認証コード",
    "確認コード",
    "認証番号",
    "確認番号",
    "ワンタイムパスワード",
    "ワンタイムコード",
    "인증번호",
    "인증코드",
    "일회용비밀번호",
    "확인코드",
];

const BARE_CODE: &[&str] = &[
    "code",
    "enter code",
    "enter the code",
    "enter your code",
    "your code",
];

const CARD_CODE_PHRASES: &[&str] = &[
    "cvv",
    "cvv2",
    "cvc",
    "cvc2",
    "csc",
    "security code",
    "card security code",
    "card verification code",
    "card verification value",
    "código de seguridad",
    "código de segurança",
    "code de sécurité",
    "cryptogramme",
    "sicherheitscode",
    "prüfnummer",
    "codice di sicurezza",
    "beveiligingscode",
    "セキュリティコード",
    "安全码",
    "보안코드",
];

/// A PIN is named only by a whole label (`PIN`, `Enter your PIN`): `Pin location` and `Search pins` are not one.
const BARE_PIN: &[&str] = &[
    "pin",
    "pin code",
    "enter pin",
    "enter your pin",
    "your pin",
    "card pin",
    "atm pin",
];

const PIN_IDS: &[&str] = &["pin", "pin code", "pin entry", "pin input", "pin field"];

fn names_a_card_code(text: &str, whole_label: bool) -> bool {
    let words = tokens(text);
    let joined = words.join(" ");
    if whole_label && BARE_PIN.contains(&joined.as_str()) {
        return true;
    }
    if !whole_label {
        let tail = joined.rsplit(" id ").next().unwrap_or(&joined);
        if PIN_IDS.contains(&tail) {
            return true;
        }
    }
    let spaced = format!(" {joined} ");
    let packed: String = phonedriver_core::text::fold(text)
        .split_whitespace()
        .collect();
    CARD_CODE_PHRASES.iter().any(|p| {
        if p.chars().any(is_cjk) {
            packed.contains(p)
        } else {
            spaced.contains(&format!(" {p} "))
        }
    })
}

fn names_a_code(text: &str, whole_label: bool) -> bool {
    let words = tokens(text);
    let spaced = format!(" {} ", words.join(" "));
    let packed: String = phonedriver_core::text::fold(text)
        .split_whitespace()
        .collect();
    if whole_label && BARE_CODE.contains(&words.join(" ").as_str()) {
        return true;
    }
    CODE_PHRASES.iter().any(|p| {
        if p.chars().any(is_cjk) {
            packed.contains(p)
        } else {
            spaced.contains(&format!(" {p} "))
        }
    })
}

/// Why plain text must not go into this field: a password field, or a text field whose label, hint or id names a
/// one-time code. The element model has no autofill hint or input type, so the words are all there is.
pub fn kind(e: &Element) -> Option<SecretKind> {
    if e.role == Role::SecureField || e.state.contains(ElementState::PASSWORD) {
        return Some(SecretKind::Password);
    }
    if !e.role.is_text_entry() {
        return None;
    }
    let words = [e.label.as_deref(), e.hint.as_deref()];
    let names = |f: fn(&str, bool) -> bool| {
        words.into_iter().flatten().any(|t| f(t, true))
            || e.id
                .as_deref()
                .is_some_and(|i| f(&i.replace('_', " "), false))
    };
    if names(names_a_code) {
        return Some(SecretKind::Code);
    }
    names(names_a_card_code).then_some(SecretKind::CardCode)
}
