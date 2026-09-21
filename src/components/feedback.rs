//! Reusable loading and error feedback components.

use leptos::prelude::*;

use crate::app::StoresStatus;

/// Full-page loading spinner with optional message.
#[component]
pub fn LoadingState(#[prop(default = "Loading...")] message: &'static str) -> impl IntoView {
    view! {
        <div class="loading-container">
            <div class="loading-spinner"></div>
            <p class="loading-text">{message}</p>
        </div>
    }
}

/// Inline loading indicator (for use inside cards/sections).
#[component]
pub fn LoadingInline(#[prop(default = "Loading...")] message: &'static str) -> impl IntoView {
    view! {
        <div class="loading-inline">
            <div class="loading-spinner-sm"></div>
            <span class="loading-text-sm">{message}</span>
        </div>
    }
}

/// Error display with retry button.
#[component]
pub fn ErrorState(
    #[prop(into)] message: String,
    #[prop(optional)] on_retry: Option<Callback<()>>,
) -> impl IntoView {
    view! {
        <div class="error-container">
            <div class="error-icon">
                <svg xmlns="http://www.w3.org/2000/svg" width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
                    <circle cx="12" cy="12" r="10"></circle>
                    <line x1="12" y1="8" x2="12" y2="12"></line>
                    <line x1="12" y1="16" x2="12.01" y2="16"></line>
                </svg>
            </div>
            <p class="error-message">{message}</p>
            {on_retry.map(|retry| view! {
                <button
                    class="ps-btn ps-btn-secondary ps-btn-sm"
                    on:click=move |_| retry.run(())
                >
                    "Try again"
                </button>
            })}
        </div>
    }
}

/// Empty state placeholder.
#[component]
pub fn EmptyState(#[prop(into)] title: String, #[prop(into)] description: String) -> impl IntoView {
    view! {
        <div class="empty-state">
            <div class="empty-state-icon">
                <svg xmlns="http://www.w3.org/2000/svg" width="48" height="48" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1" stroke-linecap="round" stroke-linejoin="round">
                    <rect x="2" y="7" width="20" height="14" rx="2" ry="2"></rect>
                    <path d="M16 21V5a2 2 0 0 0-2-2h-4a2 2 0 0 0-2 2v16"></path>
                </svg>
            </div>
            <h3 class="empty-state-title">{title}</h3>
            <p class="empty-state-description">{description}</p>
        </div>
    }
}

/// What to render on a store-scoped list page when no store is selected.
///
/// "No store selected" is four different situations, and collapsing them into
/// one onboarding empty state costs more than the `ApiError::Network` bug it
/// replaced: a user who *has* stores gets told to create one while the list is
/// still in flight, and a failed `list_stores()` looks identical to a brand-new
/// account — with no error text and no way to retry. This branches on
/// [`StoresStatus`] so each case says what is actually true.
///
/// `entity` is the plural page noun ("Invoices", "Payments").
#[component]
pub fn NoStoreSelected(
    entity: &'static str,
    status: ReadSignal<StoresStatus>,
    has_stores: Signal<bool>,
    on_retry: Callback<()>,
) -> impl IntoView {
    move || {
        match status.get() {
        // Still in flight — say nothing about the account yet.
        StoresStatus::Loading => view! { <LoadingState message="Loading stores..." /> }.into_any(),
        StoresStatus::Failed(msg) => view! {
            <ErrorState message=format!("Could not load your stores: {msg}") on_retry />
        }
        .into_any(),
        // Loaded, and the account really does have stores: "All Stores" is a
        // deliberate sidebar choice, not an unset state, so do not tell the
        // user to create anything.
        StoresStatus::Loaded if has_stores.get() => view! {
            <div class="empty-state">
                <div class="empty-state-icon">
                    <svg xmlns="http://www.w3.org/2000/svg" width="48" height="48" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1" stroke-linecap="round" stroke-linejoin="round">
                        <circle cx="11" cy="11" r="8"></circle>
                        <line x1="21" y1="21" x2="16.65" y2="16.65"></line>
                    </svg>
                </div>
                <h3>"All Stores selected"</h3>
                <p>{format!("{entity} are scoped to a single store. Pick one in the sidebar to see them.")}</p>
            </div>
        }
        .into_any(),
        // Loaded and genuinely empty — the real onboarding case.
        StoresStatus::Loaded => view! {
            <div class="empty-state">
                <div class="empty-state-icon">
                    <svg xmlns="http://www.w3.org/2000/svg" width="48" height="48" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1" stroke-linecap="round" stroke-linejoin="round">
                        <circle cx="11" cy="11" r="8"></circle>
                        <line x1="21" y1="21" x2="16.65" y2="16.65"></line>
                    </svg>
                </div>
                <h3>"No stores yet"</h3>
                <p>{format!("{entity} belong to a store. Create one to start accepting payments.")}</p>
                <a class="ps-btn ps-btn-primary ps-btn-sm" href="/evm/stores">"Go to stores"</a>
            </div>
        }
        .into_any(),
    }
    }
}

#[cfg(test)]
mod tests {
    /// The stylesheet, read at compile time so the check below is against the
    /// file that actually ships.
    const STYLES: &str = include_str!("../../styles.css");

    /// This file, likewise, so the class names in its markup are checked
    /// rather than a list of them that someone has to remember to update.
    const SOURCE: &str = include_str!("feedback.rs");

    /// Whether `styles.css` has any rule that could match `class`. Same
    /// matcher as the one in `plugin_page.rs` - see there for why it needs to
    /// be a whole-token search rather than a substring one.
    fn is_defined(class: &str) -> bool {
        let styles = strip_comments(STYLES);
        let needle = format!(".{class}");
        let mut from = 0;
        while let Some(at) = styles[from..].find(&needle) {
            let start = from + at;
            let after = styles[start + needle.len()..].chars().next();
            let before = styles[..start].chars().next_back();
            let boundary_before =
                before.is_none_or(|c| !c.is_ascii_alphanumeric() && c != '-' && c != '_');
            let boundary_after =
                after.is_none_or(|c| !c.is_ascii_alphanumeric() && c != '-' && c != '_');
            if boundary_before && boundary_after {
                return true;
            }
            from = start + 1;
        }
        false
    }

    /// `/* ... */` removed, so a class named in prose is not mistaken for a
    /// class that is styled.
    fn strip_comments(css: &str) -> String {
        let mut out = String::with_capacity(css.len());
        let mut rest = css;
        while let Some(open) = rest.find("/*") {
            out.push_str(&rest[..open]);
            match rest[open + 2..].find("*/") {
                Some(close) => rest = &rest[open + 2 + close + 2..],
                None => return out,
            }
        }
        out.push_str(rest);
        out
    }

    /// Every class these components put in the markup must exist in the
    /// stylesheet.
    ///
    /// `LoadingState` and `LoadingInline` used to fail this: `loading-container`,
    /// `loading-spinner`, `loading-text`, `loading-inline`, `loading-spinner-sm`
    /// and `loading-text-sm` were all emitted and none of them styled, so a
    /// slow-loading invoice list or invoice detail page rendered as a blank
    /// div rather than a spinner - the build stayed green because a missing
    /// class is not a compile error, only a silent one.
    #[test]
    fn every_class_these_components_emit_is_defined_in_the_stylesheet() {
        let mut missing = Vec::new();

        let code: String = SOURCE
            .lines()
            .filter(|line| !line.trim_start().starts_with("//"))
            .collect::<Vec<_>>()
            .join("\n");
        let mut rest = code.as_str();
        while let Some(at) = rest.find("class=\"") {
            rest = &rest[at + 7..];
            let Some(end) = rest.find('"') else { break };
            let (value, tail) = rest.split_at(end);
            rest = tail;
            for class in value.split_whitespace() {
                if !is_defined(class) {
                    missing.push(class.to_string());
                }
            }
        }

        missing.sort();
        missing.dedup();
        assert!(
            missing.is_empty(),
            "these classes are emitted by the shared feedback components and defined \
             nowhere in styles.css, so they style nothing: {missing:?}"
        );
    }

    /// The check above only means something if it can fail.
    #[test]
    fn a_class_the_stylesheet_does_not_define_is_detected() {
        assert!(
            !is_defined("definitely-not-a-class-in-this-stylesheet"),
            "the detector must not report an undefined class as present"
        );
        assert!(
            is_defined("loading-container"),
            "and must find one that is present"
        );
    }
}
