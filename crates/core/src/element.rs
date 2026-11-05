use std::fmt;

use bitflags::bitflags;
use serde::{Deserialize, Serialize};

use crate::geometry::Rect;

/// What an element is, independent of platform.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Role {
    Button,
    Link,
    Text,
    Heading,
    TextField,
    SearchField,
    SecureField,
    Switch,
    Checkbox,
    Radio,
    Slider,
    Image,
    List,
    Cell,
    Tab,
    Menu,
    MenuItem,
    Dialog,
    Keyboard,
    WebView,
    Alert,
    ProgressIndicator,
    Other,
}

impl Role {
    /// Maps an Android view class name (`AccessibilityNodeInfo.getClassName`).
    pub fn from_android_class(class: &str) -> Role {
        match class {
            "android.widget.Button"
            | "android.widget.ImageButton"
            | "com.google.android.material.button.MaterialButton"
            | "com.google.android.material.floatingactionbutton.FloatingActionButton" => {
                Role::Button
            }
            "android.widget.TextView" => Role::Text,
            "android.widget.EditText"
            | "android.widget.AutoCompleteTextView"
            | "android.widget.MultiAutoCompleteTextView" => Role::TextField,
            "android.widget.SearchView$SearchAutoComplete"
            | "androidx.appcompat.widget.SearchView$SearchAutoComplete" => Role::SearchField,
            "android.widget.Switch"
            | "androidx.appcompat.widget.SwitchCompat"
            | "com.google.android.material.switchmaterial.SwitchMaterial"
            | "com.google.android.material.materialswitch.MaterialSwitch"
            | "android.widget.ToggleButton" => Role::Switch,
            "android.widget.CheckBox" | "android.widget.CheckedTextView" => Role::Checkbox,
            "android.widget.RadioButton" => Role::Radio,
            "android.widget.SeekBar" | "android.widget.RatingBar" => Role::Slider,
            "android.widget.ProgressBar" => Role::ProgressIndicator,
            "android.widget.ImageView" => Role::Image,
            "android.widget.ListView"
            | "android.widget.GridView"
            | "androidx.recyclerview.widget.RecyclerView"
            | "android.support.v7.widget.RecyclerView" => Role::List,
            "android.webkit.WebView" => Role::WebView,
            "android.widget.Spinner" => Role::Menu,
            "android.app.ActionBar$Tab" | "com.google.android.material.tabs.TabLayout$TabView" => {
                Role::Tab
            }
            "android.app.AlertDialog" => Role::Dialog,
            _ => Role::Other,
        }
    }

    /// Maps an `XCUIElementType` name.
    pub fn from_xcui(ty: &str) -> Role {
        match ty.strip_prefix("XCUIElementType").unwrap_or(ty) {
            "Button" | "Icon" => Role::Button,
            "Link" => Role::Link,
            "StaticText" => Role::Text,
            "TextField" | "TextView" => Role::TextField,
            "SearchField" => Role::SearchField,
            "SecureTextField" => Role::SecureField,
            "Switch" | "Toggle" => Role::Switch,
            "CheckBox" => Role::Checkbox,
            "RadioButton" => Role::Radio,
            "Slider" | "Stepper" => Role::Slider,
            "Image" => Role::Image,
            "Table" | "CollectionView" | "Outline" => Role::List,
            "Cell" => Role::Cell,
            "Tab" => Role::Tab,
            "Menu" | "MenuBar" => Role::Menu,
            "MenuItem" | "MenuBarItem" => Role::MenuItem,
            "Sheet" | "Dialog" => Role::Dialog,
            "Keyboard" => Role::Keyboard,
            "WebView" => Role::WebView,
            "Alert" => Role::Alert,
            "ActivityIndicator" | "ProgressIndicator" => Role::ProgressIndicator,
            _ => Role::Other,
        }
    }

    /// The word used in rendered snapshots.
    pub fn name(self) -> &'static str {
        match self {
            Role::Button => "button",
            Role::Link => "link",
            Role::Text => "text",
            Role::Heading => "heading",
            Role::TextField => "textbox",
            Role::SearchField => "searchbox",
            Role::SecureField => "password",
            Role::Switch => "switch",
            Role::Checkbox => "checkbox",
            Role::Radio => "radio",
            Role::Slider => "slider",
            Role::Image => "image",
            Role::List => "list",
            Role::Cell => "cell",
            Role::Tab => "tab",
            Role::Menu => "menu",
            Role::MenuItem => "menuitem",
            Role::Dialog => "dialog",
            Role::Keyboard => "keyboard",
            Role::WebView => "webview",
            Role::Alert => "alert",
            Role::ProgressIndicator => "progress",
            Role::Other => "group",
        }
    }

    pub fn is_text_entry(self) -> bool {
        matches!(
            self,
            Role::TextField | Role::SearchField | Role::SecureField
        )
    }
}

bitflags! {
    /// Element state, rendered as words in snapshots.
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
    pub struct ElementState: u16 {
        const ENABLED = 1;
        const FOCUSED = 1 << 1;
        const SELECTED = 1 << 2;
        const CHECKED = 1 << 3;
        const CHECKABLE = 1 << 4;
        const EXPANDED = 1 << 5;
        const COLLAPSED = 1 << 6;
        const PASSWORD = 1 << 7;
    }
}

bitflags! {
    /// What an agent can do to an element.
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
    pub struct Actions: u16 {
        const TAP = 1;
        const LONG_PRESS = 1 << 1;
        const EDIT = 1 << 2;
        const SCROLL_UP = 1 << 3;
        const SCROLL_DOWN = 1 << 4;
        const SCROLL_LEFT = 1 << 5;
        const SCROLL_RIGHT = 1 << 6;
    }
}

impl Actions {
    pub const SCROLL: Actions = Actions::SCROLL_UP
        .union(Actions::SCROLL_DOWN)
        .union(Actions::SCROLL_LEFT)
        .union(Actions::SCROLL_RIGHT);
}

/// A row and column within a list, from collection item info.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ItemPos {
    pub row: u32,
    pub col: u32,
}

/// Where an element came from.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Source {
    Tree,
    Ocr,
}

/// A session-stable element name, rendered `@eN`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Ref(pub u32);

impl fmt::Display for Ref {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "e{}", self.0)
    }
}

impl Serialize for Ref {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.collect_str(self)
    }
}

impl<'de> Deserialize<'de> for Ref {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let s = String::deserialize(d)?;
        s.trim_start_matches('@')
            .strip_prefix('e')
            .and_then(|n| n.parse().ok())
            .map(Ref)
            .ok_or_else(|| serde::de::Error::custom(format!("not a ref: {s:?}")))
    }
}

pub type ElementIdx = usize;

/// One node of the pruned UI.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Element {
    pub r#ref: Ref,
    pub role: Role,
    pub label: Option<String>,
    pub value: Option<String>,
    pub hint: Option<String>,
    pub id: Option<String>,
    pub bounds: Rect,
    pub state: ElementState,
    pub actions: Actions,
    pub position: Option<ItemPos>,
    pub source: Source,
    pub children: Vec<ElementIdx>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn android_classes() {
        let table = [
            ("android.widget.Button", Role::Button),
            ("android.widget.ImageButton", Role::Button),
            (
                "com.google.android.material.button.MaterialButton",
                Role::Button,
            ),
            ("android.widget.TextView", Role::Text),
            ("android.widget.EditText", Role::TextField),
            ("android.widget.AutoCompleteTextView", Role::TextField),
            (
                "android.widget.SearchView$SearchAutoComplete",
                Role::SearchField,
            ),
            ("android.widget.Switch", Role::Switch),
            ("androidx.appcompat.widget.SwitchCompat", Role::Switch),
            (
                "com.google.android.material.switchmaterial.SwitchMaterial",
                Role::Switch,
            ),
            ("android.widget.ToggleButton", Role::Switch),
            ("android.widget.CheckBox", Role::Checkbox),
            ("android.widget.CheckedTextView", Role::Checkbox),
            ("android.widget.RadioButton", Role::Radio),
            ("android.widget.SeekBar", Role::Slider),
            ("android.widget.RatingBar", Role::Slider),
            ("android.widget.ProgressBar", Role::ProgressIndicator),
            ("android.widget.ImageView", Role::Image),
            ("android.widget.ListView", Role::List),
            ("android.widget.GridView", Role::List),
            ("androidx.recyclerview.widget.RecyclerView", Role::List),
            ("android.webkit.WebView", Role::WebView),
            ("android.widget.Spinner", Role::Menu),
            ("android.widget.LinearLayout", Role::Other),
            ("com.example.FancyThing", Role::Other),
        ];
        assert_eq!(table.len(), 25);
        for (class, role) in table {
            assert_eq!(Role::from_android_class(class), role, "{class}");
        }
    }

    #[test]
    fn xcui_types() {
        let table = [
            ("XCUIElementTypeButton", Role::Button),
            ("XCUIElementTypeLink", Role::Link),
            ("XCUIElementTypeStaticText", Role::Text),
            ("XCUIElementTypeTextField", Role::TextField),
            ("XCUIElementTypeTextView", Role::TextField),
            ("XCUIElementTypeSearchField", Role::SearchField),
            ("XCUIElementTypeSecureTextField", Role::SecureField),
            ("XCUIElementTypeSwitch", Role::Switch),
            ("XCUIElementTypeToggle", Role::Switch),
            ("XCUIElementTypeCheckBox", Role::Checkbox),
            ("XCUIElementTypeRadioButton", Role::Radio),
            ("XCUIElementTypeSlider", Role::Slider),
            ("XCUIElementTypeImage", Role::Image),
            ("XCUIElementTypeTable", Role::List),
            ("XCUIElementTypeCollectionView", Role::List),
            ("XCUIElementTypeCell", Role::Cell),
            ("XCUIElementTypeAlert", Role::Alert),
            ("XCUIElementTypeKeyboard", Role::Keyboard),
            ("XCUIElementTypeActivityIndicator", Role::ProgressIndicator),
            ("XCUIElementTypeOther", Role::Other),
        ];
        assert_eq!(table.len(), 20);
        for (ty, role) in table {
            assert_eq!(Role::from_xcui(ty), role, "{ty}");
        }
    }
}
