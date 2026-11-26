//! System settings a device offers as typed reads and writes, and the search over them (decision D57).

use std::sync::Arc;

/// One setting. `key` is what a read or a write names.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SettingItem {
    pub key: String,
    pub purpose: String,
    /// What a write accepts, in the device's words: `true | false`, `INTEGER(min=0, max=100)`.
    pub values: String,
    pub writable: bool,
    /// The device says a person must confirm a change to this setting.
    pub confirm: bool,
    /// Index into `SettingsIndex::screens`.
    pub screen: usize,
}

/// One screen of the settings app. `link` opens it; a screen that needs an item (one app, one SIM) has none.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SettingScreen {
    pub key: String,
    pub about: String,
    pub link: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SettingsIndex {
    pub items: Vec<SettingItem>,
    pub screens: Vec<SettingScreen>,
}

/// What `Device::setting` is asked.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SettingOp {
    Index,
    Get(String),
    Set { key: String, value: String },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SettingReply {
    Index(Arc<SettingsIndex>),
    /// The value now, and the device's note on it (why it cannot be read, its unit).
    Value {
        value: String,
        hint: Option<String>,
    },
}

/// The lines `find` prints, and how many more matched.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Found {
    pub lines: Vec<String>,
    pub more: usize,
}

const PURPOSE_CHARS: usize = 110;

fn cut(text: &str, chars: usize) -> String {
    let text = text.trim();
    if text.chars().count() <= chars {
        return text.to_string();
    }
    let head: String = text.chars().take(chars).collect();
    format!("{}…", head.trim_end())
}

fn first_sentence(text: &str) -> &str {
    text.split_once(". ").map_or(text, |(head, _)| head)
}

fn title(about: &str) -> &str {
    first_sentence(about).trim().trim_end_matches('.')
}

fn fold(text: &str) -> String {
    text.split_whitespace()
        .map(str::to_lowercase)
        .collect::<Vec<_>>()
        .join(" ")
}

const TITLE_CHARS: usize = 60;

impl SettingScreen {
    /// The first sentence of what the screen is about, which is the title it shows when it has one.
    pub fn title(&self) -> String {
        cut(title(&self.about), TITLE_CHARS)
    }
}

impl SettingsIndex {
    pub fn item(&self, key: &str) -> Option<&SettingItem> {
        self.items.iter().find(|i| i.key == key)
    }

    pub fn screen(&self, key: &str) -> Option<&SettingScreen> {
        self.screens.iter().find(|s| s.key == key)
    }

    /// The screen `name` means: the screen with that key, the screen a setting's key (`<screen>/<setting>`) sits on,
    /// or the one screen whose title is `name` (case and spacing ignored). A title two screens share names neither.
    pub fn named(&self, name: &str) -> Option<&SettingScreen> {
        if let Some(screen) = self.screen(name) {
            return Some(screen);
        }
        if let Some(item) = self.item(name) {
            return self.screens.get(item.screen);
        }
        if let Some(screen) = name.split_once('/').and_then(|(key, _)| self.screen(key)) {
            return Some(screen);
        }
        let wanted = fold(name);
        if wanted.is_empty() {
            return None;
        }
        let mut titled = self
            .screens
            .iter()
            .filter(|s| fold(title(&s.about)) == wanted);
        match (titled.next(), titled.next()) {
            (Some(one), None) => Some(one),
            _ => None,
        }
    }

    /// At most `limit` screens that can be opened and hold every word of `name` in their key or in what they are
    /// about (case ignored; `-`, `_` and `/` part words as spaces do), a screen whose title is `name` first.
    pub fn near(&self, name: &str, limit: usize) -> Vec<&SettingScreen> {
        let spaced = name.replace(['-', '_', '/'], " ");
        let wanted = fold(&spaced);
        if wanted.is_empty() {
            return Vec::new();
        }
        let mut near: Vec<&SettingScreen> = self
            .screens
            .iter()
            .filter(|s| s.link.is_some())
            .filter(|s| {
                let text = format!("{} {}", s.key, s.about).to_lowercase();
                wanted.split(' ').all(|w| text.contains(w))
            })
            .collect();
        near.sort_by_key(|s| fold(title(&s.about)) != wanted);
        near.truncate(limit);
        near
    }

    /// At most `limit` settings, then `screen_limit` screens, that hold every word of `query` (case ignored). A word in a setting's purpose
    /// counts for more than one in its key, and that for more than one in its screen's description.
    pub fn find(&self, query: &str, limit: usize, screen_limit: usize) -> Found {
        let words: Vec<String> = query.split_whitespace().map(str::to_lowercase).collect();
        if words.is_empty() {
            return Found::default();
        }
        let mut hits: Vec<(usize, &SettingItem)> = self
            .items
            .iter()
            .filter_map(|item| {
                let said = item.purpose.to_lowercase();
                let key = item.key.to_lowercase();
                let around = self
                    .screens
                    .get(item.screen)
                    .map(|s| s.about.to_lowercase())
                    .unwrap_or_default();
                let mut score = 0;
                for w in &words {
                    score += match w.as_str() {
                        w if said.contains(w) => 3,
                        w if key.contains(w) => 2,
                        w if around.contains(w) => 1,
                        _ => return None,
                    };
                }
                Some((score, item))
            })
            .collect();
        hits.sort_by_key(|hit| std::cmp::Reverse(hit.0));
        let screens: Vec<&SettingScreen> = self
            .screens
            .iter()
            .filter(|s| s.link.is_some())
            .filter(|s| {
                let text = format!("{} {}", s.key, s.about).to_lowercase();
                words.iter().all(|w| text.contains(w.as_str()))
            })
            .collect();
        let mut lines: Vec<String> = hits
            .iter()
            .take(limit)
            .map(|(_, item)| {
                let values = if !item.writable {
                    "read-only".to_string()
                } else if item.confirm {
                    format!("{}, the user confirms", item.values)
                } else {
                    item.values.clone()
                };
                format!(
                    "{}  [{}]  {}",
                    item.key,
                    values,
                    cut(&item.purpose, PURPOSE_CHARS)
                )
            })
            .collect();
        lines.extend(screens.iter().take(screen_limit).map(|s| {
            format!(
                "screen {}  {}",
                s.key,
                cut(first_sentence(&s.about), PURPOSE_CHARS)
            )
        }));
        Found {
            lines,
            more: hits.len().saturating_sub(limit) + screens.len().saturating_sub(screen_limit),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn index() -> SettingsIndex {
        let item =
            |key: &str, purpose: &str, values: &str, writable: bool, screen: usize| SettingItem {
                key: key.into(),
                purpose: purpose.into(),
                values: values.into(),
                writable,
                confirm: false,
                screen,
            };
        SettingsIndex {
            screens: vec![
                SettingScreen {
                    key: "dark_ui_mode".into(),
                    about: "Dark theme. Allows users to enable and customize the system-wide dark theme.".into(),
                    link: Some("intent:#Intent;S.screen_key=dark_ui_mode;end".into()),
                },
                SettingScreen {
                    key: "sound_screen".into(),
                    about: "Sound and vibration. Volume levels.".into(),
                    link: Some("intent:#Intent;S.screen_key=sound_screen;end".into()),
                },
                SettingScreen {
                    key: "app_notifications".into(),
                    about: "Notifications of one app, dark or not.".into(),
                    link: None,
                },
            ],
            items: vec![
                item(
                    "dark_ui_mode/dark_ui_auto_mode",
                    "The schedule for switching it on and off.",
                    "None | Sunset to sunrise",
                    true,
                    0,
                ),
                item(
                    "dark_ui_mode/dark_ui_activated",
                    "Whether the system dark theme is manually enabled",
                    "true | false",
                    true,
                    0,
                ),
                item(
                    "sound_screen/media_volume",
                    "The system volume level for media playback. Higher is louder.",
                    "INTEGER(min=0, max=15)",
                    true,
                    1,
                ),
                SettingItem {
                    confirm: true,
                    ..item(
                        "sound_screen/cycle_count",
                        "The battery charge cycle count",
                        "",
                        false,
                        1,
                    )
                },
            ],
        }
    }

    #[test]
    fn a_setting_that_names_the_word_itself_comes_before_one_that_only_sits_on_such_a_screen() {
        let found = index().find("dark", 12, 3);
        assert_eq!(
            found.lines,
            [
                "dark_ui_mode/dark_ui_activated  [true | false]  Whether the system dark theme is manually enabled",
                "dark_ui_mode/dark_ui_auto_mode  [None | Sunset to sunrise]  The schedule for switching it on and off.",
                "screen dark_ui_mode  Dark theme",
            ]
        );
        assert_eq!(found.more, 0);
    }

    #[test]
    fn every_word_must_match_and_case_is_ignored() {
        let found = index().find("Media VOLUME", 12, 3);
        assert_eq!(found.lines.len(), 1, "{:?}", found.lines);
        assert!(found.lines[0].starts_with("sound_screen/media_volume  [INTEGER(min=0, max=15)]"));
        assert!(index().find("volume teapot", 12, 3).lines.is_empty());
        assert!(index().find("  ", 12, 3).lines.is_empty());
    }

    #[test]
    fn a_read_only_setting_says_so_and_a_screen_that_cannot_be_opened_is_left_out() {
        let found = index().find("cycle", 12, 3);
        assert_eq!(
            found.lines,
            ["sound_screen/cycle_count  [read-only]  The battery charge cycle count"]
        );
        let found = index().find("notifications", 12, 3);
        assert!(found.lines.is_empty(), "{:?}", found.lines);
    }

    #[test]
    fn a_screen_is_named_by_its_key_by_a_setting_on_it_or_by_the_title_it_shows() {
        let index = index();
        let key = |name: &str| index.named(name).map(|s| s.key.as_str());
        assert_eq!(key("dark_ui_mode"), Some("dark_ui_mode"));
        assert_eq!(key("dark_ui_mode/dark_ui_activated"), Some("dark_ui_mode"));
        assert_eq!(key("sound_screen/no_such_setting"), Some("sound_screen"));
        assert_eq!(key("Dark theme"), Some("dark_ui_mode"));
        assert_eq!(key("sound and  vibration"), Some("sound_screen"));
        assert_eq!(key("dark"), None);
        assert_eq!(key("volume"), None);
        assert_eq!(key(""), None);
    }

    #[test]
    fn a_title_two_screens_share_names_neither() {
        let mut index = index();
        index.screens[1].about = "Dark theme. Another screen with that title.".into();
        assert_eq!(index.named("Dark theme"), None);
        assert_eq!(index.named("dark_ui_mode").unwrap().key, "dark_ui_mode");
    }

    #[test]
    fn near_lists_the_screens_that_open_and_hold_every_word_a_title_first() {
        let index = index();
        let keys = |name: &str| -> Vec<&str> {
            index.near(name, 4).iter().map(|s| s.key.as_str()).collect()
        };
        assert_eq!(keys("dark"), ["dark_ui_mode"]);
        assert_eq!(keys("sound-screen"), ["sound_screen"]);
        assert_eq!(keys("Volume_levels"), ["sound_screen"]);
        assert!(keys("battery").is_empty());
        assert!(keys(" - ").is_empty());
        let mut two = index.clone();
        two.screens[2].link = Some("intent:#Intent;S.screen_key=app_notifications;end".into());
        two.screens[2].about = "Sound. Notifications of one app, dark or not.".into();
        let keys: Vec<&str> = two
            .near("sound", 4)
            .iter()
            .map(|s| s.key.as_str())
            .collect();
        assert_eq!(keys, ["app_notifications", "sound_screen"]);
        assert_eq!(two.near("sound", 1).len(), 1);
    }

    #[test]
    fn what_did_not_fit_is_counted() {
        let found = index().find("dark", 1, 3);
        assert_eq!(found.lines.len(), 2, "{:?}", found.lines);
        assert_eq!(found.more, 1);
    }
}
