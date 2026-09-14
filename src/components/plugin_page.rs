//! Renders a plugin's page descriptor tree into ui-kit markup (RCS-302).
//!
//! `render` is a `match` over [`PageElement`], not a set of `#[component]`s:
//! the tree is recursive and arrives over the wire as data, so there is no
//! fixed set of props `view!` could be written against ahead of time. A
//! plain function recursing into its own children is the direct translation
//! of "draw whatever tree the host sent."
//!
//! Static rendering only: no `on:click`, no bound `value`. A button renders
//! and does nothing yet - that is the next slice, not this one.
//!
//! Class names are written as literal strings, one per match arm, rather
//! than built with `format!` (joining a fixed prefix to a tone or variant
//! name at runtime). `scripts/check-classes-styled.py` finds every `ps-`
//! class by scanning source for quoted strings, so a class assembled at
//! runtime is invisible to it; see that script's own comment on
//! `if cond { "a" } else { "b" }` for the same rule.
//!
//! The `PageElement::Unknown` arm is the one ui-kit renames cannot delete:
//! it is what a newer plugin's element type becomes when this client does
//! not recognise it, and it must render a visible placeholder, never
//! nothing - a silently blank panel beside a paywall is the failure this
//! prevents. The match below has no wildcard arm, so removing this case is a
//! compile error, not a page that quietly goes blank.

use leptos::prelude::*;

use crate::api::{
    Badge, Button, ButtonVariant, Card, Direction, Form, Grid, Input, Notice, PageElement, Row,
    Section, Select, Stack, Table, Tabs, Tone,
};

/// Draws one node of a plugin page, recursing into its children.
pub fn render(element: &PageElement) -> AnyView {
    match element {
        PageElement::Card(card) => render_card(card),
        PageElement::Badge(badge) => render_badge(badge),
        PageElement::Button(button) => render_button(button),
        PageElement::Table(table) => render_table(table),
        PageElement::Form(form) => render_form(form),
        PageElement::Input(input) => render_input(input),
        PageElement::Select(select) => render_select(select),
        PageElement::Notice(notice) => render_notice(notice),
        PageElement::Tabs(tabs) => render_tabs(tabs),
        PageElement::Stack(stack) => render_stack(stack),
        PageElement::Row(row) => render_row(row),
        PageElement::Grid(grid) => render_grid(grid),
        PageElement::Section(section) => render_section(section),
        PageElement::Unknown => render_placeholder(),
    }
}

// `Vec<AnyView>` rather than `-> impl IntoView`: edition 2024 has `impl
// Trait` in return position capture all in-scope lifetimes by default, which
// would tie this opaque type to `children`'s borrow even though every
// `AnyView` it contains is already fully owned. A concrete `Vec<AnyView>`
// carries no such lifetime and is what `render`'s callers actually need,
// since they immediately wrap it in their own `AnyView`.
fn render_children(children: &[PageElement]) -> Vec<AnyView> {
    children.iter().map(render).collect()
}

fn render_card(card: &Card) -> AnyView {
    let title = card.title.clone();
    let body = render_children(&card.children);
    match title {
        Some(title) => view! {
            <div class="ps-card">
                <div class="ps-card-header"><h3>{title}</h3></div>
                <div class="ps-card-body">{body}</div>
            </div>
        }
        .into_any(),
        None => view! { <div class="ps-card ps-card-padded">{body}</div> }.into_any(),
    }
}

/// The full `ps-badge`/`ps-badge-{tone}` pair for one [`Tone`]. Kept as a
/// `match` over literal strings, not `format!`, so every class is visible to
/// `scripts/check-classes-styled.py` - see the module doc.
fn badge_class(tone: Tone) -> &'static str {
    match tone {
        Tone::Neutral => "ps-badge ps-badge-neutral",
        Tone::Info => "ps-badge ps-badge-info",
        Tone::Success => "ps-badge ps-badge-success",
        Tone::Warning => "ps-badge ps-badge-warning",
        Tone::Danger => "ps-badge ps-badge-danger",
    }
}

fn render_badge(badge: &Badge) -> AnyView {
    view! { <span class=badge_class(badge.tone)>{badge.text.clone()}</span> }.into_any()
}

/// See [`badge_class`] for why this is a `match` over literal pairs.
fn button_class(variant: ButtonVariant) -> &'static str {
    match variant {
        ButtonVariant::Primary => "ps-btn ps-btn-primary",
        ButtonVariant::Secondary => "ps-btn ps-btn-secondary",
        ButtonVariant::Outline => "ps-btn ps-btn-outline",
        ButtonVariant::Ghost => "ps-btn ps-btn-ghost",
        ButtonVariant::Danger => "ps-btn ps-btn-danger",
    }
}

fn render_button(button: &Button) -> AnyView {
    view! { <button class=button_class(button.variant)>{button.label.clone()}</button> }.into_any()
}

fn render_table(table: &Table) -> AnyView {
    let headers = table.headers.clone();
    let rows = table.rows.clone();
    view! {
        <table class="ps-table">
            <thead>
                <tr>
                    {headers
                        .into_iter()
                        .map(|h| view! { <th>{h}</th> })
                        .collect::<Vec<_>>()}
                </tr>
            </thead>
            <tbody>
                {rows
                    .into_iter()
                    .map(|row: Vec<String>| {
                        view! {
                            <tr>
                                {row
                                    .into_iter()
                                    .map(|cell| view! { <td>{cell}</td> })
                                    .collect::<Vec<_>>()}
                            </tr>
                        }
                    })
                    .collect::<Vec<_>>()}
            </tbody>
        </table>
    }
    .into_any()
}

fn render_form(form: &Form) -> AnyView {
    view! { <form class="ps-form">{render_children(&form.children)}</form> }.into_any()
}

/// Static, per the module doc: a real `value`/`on:input` binding is the next
/// slice. `readonly` (not `disabled`) keeps it visually identical to a live
/// field, since "does nothing yet" is not "is unavailable".
fn render_input(input: &Input) -> AnyView {
    let label = input.label.clone();
    let placeholder = input.placeholder.clone();
    view! {
        <div class="ps-form-group">
            {label.map(|l| view! { <label class="ps-form-label">{l}</label> })}
            <input class="ps-form-input" type="text" placeholder=placeholder readonly=true />
        </div>
    }
    .into_any()
}

fn render_select(select: &Select) -> AnyView {
    let label = select.label.clone();
    let options = select.options.clone();
    view! {
        <div class="ps-form-group">
            {label.map(|l| view! { <label class="ps-form-label">{l}</label> })}
            <select class="ps-select">
                {options
                    .into_iter()
                    .map(|o| view! { <option>{o}</option> })
                    .collect::<Vec<_>>()}
            </select>
        </div>
    }
    .into_any()
}

/// See [`badge_class`] for why this is a `match` over literal pairs.
fn notice_class(tone: Tone) -> &'static str {
    match tone {
        Tone::Neutral => "ps-notice ps-notice-neutral",
        Tone::Info => "ps-notice ps-notice-info",
        Tone::Success => "ps-notice ps-notice-success",
        Tone::Warning => "ps-notice ps-notice-warning",
        Tone::Danger => "ps-notice ps-notice-danger",
    }
}

fn render_notice(notice: &Notice) -> AnyView {
    view! { <div class=notice_class(notice.tone)>{notice.text.clone()}</div> }.into_any()
}

/// See [`badge_class`] for why this is a `match` over literal pairs.
fn tab_class(active: bool) -> &'static str {
    if active {
        "ps-tabs-tab ps-tabs-tab-active"
    } else {
        "ps-tabs-tab"
    }
}

/// No click handling yet (see the module doc), so there is no state to pick
/// which tab is open - the first tab's content is what renders, and every
/// label is shown so the page does not look like it has only one tab.
fn render_tabs(tabs: &Tabs) -> AnyView {
    let active_content = tabs.tabs.first().map(|tab| render_children(&tab.content));
    let labels = tabs.tabs.clone();
    view! {
        <div class="ps-tabs">
            <div class="ps-tabs-list">
                {labels
                    .into_iter()
                    .enumerate()
                    .map(|(i, tab)| { view! { <span class=tab_class(i == 0)>{tab.label}</span> } })
                    .collect::<Vec<_>>()}
            </div>
            <div class="ps-tabs-panel">{active_content}</div>
        </div>
    }
    .into_any()
}

/// See [`badge_class`] for why this is a `match` over literal pairs.
fn stack_class(direction: Direction) -> &'static str {
    match direction {
        Direction::Column => "ps-stack ps-stack-col",
        Direction::Row => "ps-stack ps-stack-row",
    }
}

fn render_stack(stack: &Stack) -> AnyView {
    view! {
        <div class=stack_class(stack.direction)>{render_children(&stack.children)}</div>
    }
    .into_any()
}

fn render_row(row: &Row) -> AnyView {
    view! { <div class="ps-row">{render_children(&row.children)}</div> }.into_any()
}

/// Column count is per-instance data, not one of a fixed set of variants, so
/// it is the one property here carried as an inline style rather than a
/// class - the same choice ui-kit's own `Grid` component makes.
fn render_grid(grid: &Grid) -> AnyView {
    let style = format!(
        "grid-template-columns: repeat({}, minmax(0, 1fr));",
        grid.columns.max(1)
    );
    view! {
        <div class="ps-grid" style=style>{render_children(&grid.children)}</div>
    }
    .into_any()
}

fn render_section(section: &Section) -> AnyView {
    let title = section.title.clone();
    view! {
        <section class="ps-section">
            {title.map(|t| view! { <h2 class="ps-section-title">{t}</h2> })}
            {render_children(&section.children)}
        </section>
    }
    .into_any()
}

/// The visible placeholder for an element this client does not recognise.
/// See the module doc: this arm existing (and staying non-blank) is the
/// property RCS-302 exists to guarantee.
fn render_placeholder() -> AnyView {
    view! { <div class="ps-placeholder">"Unsupported plugin element"</div> }.into_any()
}

#[cfg(test)]
mod tests {
    use super::*;

    // These assert the exact class pair per variant rather than a
    // starts_with prefix check: check-classes-styled.py finds ps- classes by
    // scanning source for quoted strings, so a quoted partial prefix ending
    // in a bare hyphen would itself read as an unstyled class. Asserting the
    // full string sidesteps that and is a stronger check anyway.
    #[test]
    fn every_badge_tone_has_its_own_class_pair() {
        assert_eq!(badge_class(Tone::Neutral), "ps-badge ps-badge-neutral");
        assert_eq!(badge_class(Tone::Info), "ps-badge ps-badge-info");
        assert_eq!(badge_class(Tone::Success), "ps-badge ps-badge-success");
        assert_eq!(badge_class(Tone::Warning), "ps-badge ps-badge-warning");
        assert_eq!(badge_class(Tone::Danger), "ps-badge ps-badge-danger");
    }

    #[test]
    fn every_button_variant_has_its_own_class_pair() {
        assert_eq!(
            button_class(ButtonVariant::Primary),
            "ps-btn ps-btn-primary"
        );
        assert_eq!(
            button_class(ButtonVariant::Secondary),
            "ps-btn ps-btn-secondary"
        );
        assert_eq!(
            button_class(ButtonVariant::Outline),
            "ps-btn ps-btn-outline"
        );
        assert_eq!(button_class(ButtonVariant::Ghost), "ps-btn ps-btn-ghost");
        assert_eq!(button_class(ButtonVariant::Danger), "ps-btn ps-btn-danger");
    }

    #[test]
    fn every_notice_tone_has_its_own_class_pair() {
        assert_eq!(notice_class(Tone::Neutral), "ps-notice ps-notice-neutral");
        assert_eq!(notice_class(Tone::Info), "ps-notice ps-notice-info");
        assert_eq!(notice_class(Tone::Success), "ps-notice ps-notice-success");
        assert_eq!(notice_class(Tone::Warning), "ps-notice ps-notice-warning");
        assert_eq!(notice_class(Tone::Danger), "ps-notice ps-notice-danger");
    }

    #[test]
    fn stack_direction_selects_the_matching_class() {
        assert_eq!(stack_class(Direction::Column), "ps-stack ps-stack-col");
        assert_eq!(stack_class(Direction::Row), "ps-stack ps-stack-row");
    }

    #[test]
    fn only_the_first_tab_is_marked_active() {
        assert_eq!(tab_class(true), "ps-tabs-tab ps-tabs-tab-active");
        assert_eq!(tab_class(false), "ps-tabs-tab");
    }

    // `RenderHtml::to_html` panics unless leptos's `ssr` feature is active
    // (it is a runtime check inside `tachys::view::any_view`, not a compile
    // gate). That feature is a dev-dependency only - see the `Cargo.toml`
    // comment - so it is never present in the wasm bundle this crate ships;
    // it exists solely so these host-target tests can inspect real markup
    // instead of asserting on the string literals inside `view!` by eye.
    fn to_html(element: &PageElement) -> String {
        use leptos::prelude::RenderHtml;
        render(element).to_html()
    }

    fn from_json(json: &str) -> PageElement {
        serde_json::from_str(json).unwrap()
    }

    // Every test below is the ticket's round-trip in full: a JSON string (the
    // wire shape a plugin actually sends) deserializes into `PageElement`,
    // `render` turns that into a real view tree, and `to_html` renders that
    // tree so the assertion is against markup a browser would receive - not
    // against a class-literal helper or a hand-built enum value. This is what
    // catches a `view!` class typo that `badge_class`/`button_class`/etc.
    // above cannot: those only check the helper functions, not that `render`
    // actually wires the helper's output into the tag it returns.

    #[test]
    fn card_round_trips_with_and_without_a_title() {
        let with_title = from_json(r#"{"type": "card", "title": "Balance", "children": []}"#);
        let html = to_html(&with_title);
        assert!(html.contains("ps-card"));
        assert!(html.contains("ps-card-header"));
        assert!(html.contains("ps-card-body"));
        assert!(html.contains("Balance"));

        let untitled = from_json(r#"{"type": "card", "children": []}"#);
        let html = to_html(&untitled);
        assert!(html.contains("ps-card ps-card-padded"));
    }

    #[test]
    fn badge_round_trips() {
        let element = from_json(r#"{"type": "badge", "text": "Paid", "tone": "success"}"#);
        let html = to_html(&element);
        assert!(html.contains("ps-badge ps-badge-success"));
        assert!(html.contains("Paid"));
    }

    #[test]
    fn button_round_trips() {
        let element = from_json(r#"{"type": "button", "label": "Retry", "variant": "danger"}"#);
        let html = to_html(&element);
        assert!(html.contains("ps-btn ps-btn-danger"));
        assert!(html.contains("Retry"));
    }

    #[test]
    fn table_round_trips() {
        let element = from_json(r#"{"type": "table", "headers": ["Amount"], "rows": [["1.00"]]}"#);
        let html = to_html(&element);
        assert!(html.contains("ps-table"));
        assert!(html.contains("Amount"));
        assert!(html.contains("1.00"));
    }

    #[test]
    fn form_round_trips() {
        let element = from_json(r#"{"type": "form", "children": []}"#);
        let html = to_html(&element);
        assert!(html.contains("ps-form"));
    }

    #[test]
    fn input_round_trips() {
        let element =
            from_json(r#"{"type": "input", "label": "Email", "placeholder": "you@example.com"}"#);
        let html = to_html(&element);
        assert!(html.contains("ps-form-group"));
        assert!(html.contains("ps-form-label"));
        assert!(html.contains("ps-form-input"));
        assert!(html.contains("Email"));
    }

    #[test]
    fn select_round_trips() {
        let element =
            from_json(r#"{"type": "select", "label": "Chain", "options": ["ETH", "MATIC"]}"#);
        let html = to_html(&element);
        assert!(html.contains("ps-form-group"));
        assert!(html.contains("ps-select"));
        assert!(html.contains("ETH"));
    }

    #[test]
    fn notice_round_trips() {
        let element = from_json(r#"{"type": "notice", "text": "Rate limited", "tone": "warning"}"#);
        let html = to_html(&element);
        assert!(html.contains("ps-notice ps-notice-warning"));
        assert!(html.contains("Rate limited"));
    }

    #[test]
    fn tabs_round_trips() {
        let element = from_json(
            r#"{"type": "tabs", "tabs": [
                {"label": "Overview", "content": [{"type": "badge", "text": "Live", "tone": "info"}]},
                {"label": "History", "content": []}
            ]}"#,
        );
        let html = to_html(&element);
        assert!(html.contains("ps-tabs"));
        assert!(html.contains("ps-tabs-list"));
        assert!(html.contains("ps-tabs-tab ps-tabs-tab-active"));
        assert!(html.contains("ps-tabs-panel"));
        assert!(html.contains("Overview"));
        assert!(html.contains("History"));
        assert!(html.contains("ps-badge ps-badge-info"));
    }

    #[test]
    fn stack_round_trips() {
        let element = from_json(
            r#"{"type": "stack", "direction": "row", "children": [{"type": "badge", "text": "A", "tone": "neutral"}]}"#,
        );
        let html = to_html(&element);
        assert!(html.contains("ps-stack ps-stack-row"));
        assert!(html.contains("ps-badge"));
    }

    #[test]
    fn row_round_trips() {
        let element = from_json(
            r#"{"type": "row", "children": [{"type": "badge", "text": "A", "tone": "neutral"}]}"#,
        );
        let html = to_html(&element);
        assert!(html.contains("ps-row"));
        assert!(html.contains("ps-badge"));
    }

    #[test]
    fn grid_round_trips() {
        let element = from_json(r#"{"type": "grid", "columns": 3, "children": []}"#);
        let html = to_html(&element);
        assert!(html.contains("ps-grid"));
        assert!(html.contains("repeat(3, minmax(0, 1fr))"));
    }

    #[test]
    fn section_round_trips() {
        let element = from_json(
            r#"{"type": "section", "title": "Settings", "children": [{"type": "badge", "text": "A", "tone": "neutral"}]}"#,
        );
        let html = to_html(&element);
        assert!(html.contains("ps-section"));
        assert!(html.contains("ps-section-title"));
        assert!(html.contains("Settings"));
        assert!(html.contains("ps-badge"));
    }

    /// The property the module doc calls out: an element type this client
    /// does not recognise deserializes as `Unknown`, and `render` draws it
    /// as a visible placeholder rather than nothing - checked here against
    /// real rendered markup, not just the deserialized enum value. `render`'s
    /// `match` has no wildcard arm, so deleting this arm is a compile error:
    /// the strongest version of "confirm the page renders blank without it"
    /// available in a plain `match` - there is no way to silently fall
    /// through to nothing.
    #[test]
    fn an_unrecognised_element_type_becomes_the_placeholder() {
        let json = r#"{"type": "chart", "series": [1, 2, 3]}"#;
        let element: PageElement = serde_json::from_str(json).unwrap();
        assert_eq!(element, PageElement::Unknown);

        let html = to_html(&element);
        assert!(html.contains("ps-placeholder"));
        assert!(html.contains("Unsupported plugin element"));
    }
}
