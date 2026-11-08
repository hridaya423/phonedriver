//! Fields the model must not fill with plain text (decision D30): passwords and one-time codes.

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

fn names_a_code(text: &str, whole_label: bool) -> bool {
    let words = tokens(text);
    let spaced = format!(" {} ", words.join(" "));
    let packed: String = text.to_lowercase().split_whitespace().collect();
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
    let by_words = [e.label.as_deref(), e.hint.as_deref()]
        .into_iter()
        .flatten()
        .any(|t| names_a_code(t, true));
    let by_id = e.id.as_deref().is_some_and(|i| names_a_code(i, false));
    (by_words || by_id).then_some(SecretKind::Code)
}
