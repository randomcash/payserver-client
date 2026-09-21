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

    // `is_defined` and the literal-class scanner live in `crate::style_check`,
    // shared with `pages/plugin_page.rs`'s copy of this same check, so the
    // two cannot drift apart.
    use crate::style_check::{is_defined, literal_classes};

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

        for class in literal_classes(SOURCE) {
            if !is_defined(STYLES, &class) {
                missing.push(class);
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

    /// `literal_classes` only sees a class written as `class="..."`. A class
    /// built as `class={some_helper()}` would be invisible to it - exactly
    /// the gap `plugin_page.rs` closes by calling its own class-choosing
    /// functions (`tone_class`, `notice_class`, `button_class`) and checking
    /// their output directly. None of the components here choose a class that
    /// way today, so this check exists to keep that fact loud: the day one
    /// does, it must fail until the class is added to the check above,
    /// the same way `plugin_page.rs`'s `from_functions` list is kept in step
    /// with the enums it reads.
    #[test]
    fn no_class_is_computed_outside_the_literal_scan_above() {
        // Built from parts, none of which spell `class="` on their own, so
        // this line does not itself contain the pattern it is checking for -
        // `SOURCE` is this whole file, tests included.
        let dynamic_class_attribute = ["class", "=", "{"].concat();

        let code: String = SOURCE
            .lines()
            .filter(|line| !line.trim_start().starts_with("//"))
            .collect::<Vec<_>>()
            .join("\n");
        assert!(
            !code.contains(&dynamic_class_attribute),
            "a class attribute written as an expression rather than a literal string \
             appeared in this file - the literal-string scan above cannot see it, so \
             add its possible outputs to that scan the way plugin_page.rs's \
             `from_functions` list does, or a class here can go unstyled with a green \
             build"
        );
    }

    /// The check above only means something if it can fail.
    #[test]
    fn a_class_the_stylesheet_does_not_define_is_detected() {
        assert!(
            !is_defined(STYLES, "definitely-not-a-class-in-this-stylesheet"),
            "the detector must not report an undefined class as present"
        );
        assert!(
            is_defined(STYLES, "loading-container"),
            "and must find one that is present"
        );
    }
}
