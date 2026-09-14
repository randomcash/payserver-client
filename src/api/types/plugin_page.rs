//! The page descriptor, hand-mirrored from `payserver_plugin_api::page`.
//!
//! This crate pins `payserver-commons` by `rev` in its own `Cargo.toml`, and
//! moving that pin needs the commons PR merged first. RCS-302 adds the
//! descriptor to commons in the same session that adds this renderer, so the
//! new module cannot be on the pinned revision yet - a single autonomous run
//! cannot get a sibling-repo PR merged and then depend on that merge within
//! the same run. See `api/types/mod.rs` for the general rule (most DTOs come
//! from `api-types` directly) and its documented exception for cases like
//! this one.
//!
//! This copy must stay wire-compatible with `payserver_plugin_api::page`
//! (same `#[serde(tag = "type")]` shape) and should be deleted in favour of a
//! direct import the next time the commons `rev` bumps.

use serde::{Deserialize, Serialize};

/// Who the host says is asking. The renderer never chooses this - it only
/// draws whatever tree the host sent for the viewer it resolved.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Viewer {
    Merchant,
    Admin,
}

/// A node in a plugin page. Variant names are a public contract: when
/// ui-kit renames one of its own components, this enum keeps the old name
/// and [`crate::components::plugin_page::render`] moves the mapping in a
/// `match`, rather than the descriptor renaming to track ui-kit.
///
/// `Unknown` is what lets this client accept a page from a newer plugin: a
/// `type` this build does not recognise deserializes here instead of
/// failing the whole page. The renderer draws a visible placeholder for it -
/// never nothing.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum PageElement {
    Card(Card),
    Badge(Badge),
    Button(Button),
    Table(Table),
    Form(Form),
    Input(Input),
    Select(Select),
    Notice(Notice),
    Tabs(Tabs),
    Stack(Stack),
    Row(Row),
    Grid(Grid),
    Section(Section),
    #[serde(other)]
    Unknown,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Tone {
    #[default]
    Neutral,
    Info,
    Success,
    Warning,
    Danger,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ButtonVariant {
    #[default]
    Primary,
    Secondary,
    Outline,
    Ghost,
    Danger,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Direction {
    #[default]
    Column,
    Row,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Card {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    #[serde(default)]
    pub children: Vec<PageElement>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Badge {
    pub text: String,
    #[serde(default)]
    pub tone: Tone,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Button {
    pub label: String,
    #[serde(default)]
    pub variant: ButtonVariant,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Table {
    #[serde(default)]
    pub headers: Vec<String>,
    #[serde(default)]
    pub rows: Vec<Vec<String>>,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Form {
    #[serde(default)]
    pub children: Vec<PageElement>,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Input {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub label: Option<String>,
    #[serde(default)]
    pub placeholder: String,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Select {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub label: Option<String>,
    #[serde(default)]
    pub options: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Notice {
    pub text: String,
    #[serde(default)]
    pub tone: Tone,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Tab {
    pub label: String,
    #[serde(default)]
    pub content: Vec<PageElement>,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Tabs {
    #[serde(default)]
    pub tabs: Vec<Tab>,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Stack {
    #[serde(default)]
    pub direction: Direction,
    #[serde(default)]
    pub children: Vec<PageElement>,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Row {
    #[serde(default)]
    pub children: Vec<PageElement>,
}

fn default_columns() -> u8 {
    2
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Grid {
    #[serde(default = "default_columns")]
    pub columns: u8,
    #[serde(default)]
    pub children: Vec<PageElement>,
}

impl Default for Grid {
    fn default() -> Self {
        Self {
            columns: default_columns(),
            children: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Section {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    #[serde(default)]
    pub children: Vec<PageElement>,
}
