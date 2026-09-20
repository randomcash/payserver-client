//! Draws a plugin's page.
//!
//! Nothing in this file knows what any plugin does. A plugin ships a
//! [`PageElement`] tree and this renders it, which is the only arrangement
//! that works: a plugin cannot ship Rust into an already-compiled client,
//! and this client is public while some plugins are not.
//!
//! # Two rules that are not cosmetic
//!
//! **An unrecognised element still draws something.** The vocabulary grows,
//! and a plugin built against a newer server will send elements this build
//! has never heard of. `PageElement::Unknown` is what that becomes, and it
//! renders a visible placeholder - because the alternative is a blank space
//! where a merchant expected to see what they owe, and a silently missing
//! panel beside a paywall is the worst thing this renderer could produce.
//!
//! **A link is checked before it is followed.** A plugin names where its
//! button goes, and a plugin is not a trusted source of somewhere to send a
//! merchant with money in hand. Only a host-relative path is honoured; an
//! absolute URL, a scheme, or a protocol-relative `//host` is treated as no
//! link at all, and the button draws disabled.

use leptos::prelude::*;
use leptos_router::hooks::use_params_map;
use payserver_plugin_api::page::{
    Badge, Button, ButtonVariant, Card, Direction, Fields, Form, Grid, Input, Notice, PageElement,
    Row, Section, Select, Stack, Tab, Table, Tabs, Text, TextStyle, Tone,
};

use crate::api::ApiClient;

/// Whether a plugin-supplied link may be followed.
///
/// Only a path on this origin. Everything else is refused, and refused
/// silently in the direction that cannot hurt: the button renders disabled
/// rather than pointing somewhere else.
///
/// `//evil.example` is the case worth naming. It has no scheme, so a check
/// for `http://` misses it, and a browser reads it as protocol-relative and
/// goes to another host.
#[must_use]
pub fn safe_href(href: &str) -> Option<&str> {
    let trimmed = href.trim();
    if trimmed.starts_with('/') && !trimmed.starts_with("//") {
        Some(trimmed)
    } else {
        None
    }
}

fn tone_class(tone: Tone) -> &'static str {
    match tone {
        Tone::Neutral => "badge badge-neutral",
        Tone::Info => "badge badge-info",
        Tone::Success => "badge badge-success",
        Tone::Warning => "badge badge-warning",
        Tone::Danger => "badge badge-error",
    }
}

fn notice_class(tone: Tone) -> &'static str {
    match tone {
        Tone::Neutral => "plugin-notice plugin-notice-neutral",
        Tone::Info => "plugin-notice plugin-notice-info",
        Tone::Success => "plugin-notice plugin-notice-success",
        Tone::Warning => "plugin-notice plugin-notice-warning",
        Tone::Danger => "plugin-notice plugin-notice-danger",
    }
}

fn button_class(variant: ButtonVariant) -> &'static str {
    match variant {
        ButtonVariant::Primary => "btn btn-primary",
        ButtonVariant::Secondary => "btn btn-secondary",
        ButtonVariant::Danger => "btn btn-danger",
        ButtonVariant::Ghost | ButtonVariant::Outline => "btn btn-ghost",
    }
}

/// One element and everything under it.
fn render(element: &PageElement) -> AnyView {
    match element {
        PageElement::Badge(Badge { text, tone }) => view! {
            <span class=tone_class(*tone)>{text.clone()}</span>
        }
        .into_any(),

        // Prose. Drawn with the host's own type styles rather than anything
        // the plugin chose - a plugin says what the text is doing, never how
        // big it is, so a plugin page cannot drift away from the screens
        // beside it.
        PageElement::Text(Text { text, style }) => {
            let class = match style {
                TextStyle::Body => "plugin-text",
                TextStyle::Muted => "plugin-text plugin-text-muted",
                TextStyle::Strong => "plugin-text plugin-text-strong",
            };
            view! { <p class=class>{text.clone()}</p> }.into_any()
        }

        // Label-and-value pairs, as a definition list. A `<dl>` because that
        // is what this is, and because it lets the stylesheet collapse to one
        // column on a narrow screen without the renderer knowing the width.
        PageElement::Fields(Fields { fields }) => {
            let rows = fields
                .iter()
                .map(|field| {
                    view! {
                        <div class="plugin-field-row">
                            <dt class="plugin-field-key">{field.label.clone()}</dt>
                            <dd class="plugin-field-value">{field.value.clone()}</dd>
                        </div>
                    }
                })
                .collect_view();
            view! { <dl class="plugin-fields">{rows}</dl> }.into_any()
        }

        PageElement::Notice(Notice { text, tone }) => view! {
            <div class=notice_class(*tone) role="status">{text.clone()}</div>
        }
        .into_any(),

        PageElement::Button(Button {
            label,
            variant,
            href,
        }) => {
            let class = button_class(*variant);
            match href.as_deref().and_then(safe_href) {
                Some(target) => view! {
                    <a class=class href=target.to_string()>{label.clone()}</a>
                }
                .into_any(),
                // No action system, and no link we will follow: the control
                // exists on the page, so it is drawn - visibly unavailable
                // rather than live and inert.
                None => view! {
                    <button class=class disabled=true>{label.clone()}</button>
                }
                .into_any(),
            }
        }

        PageElement::Card(Card { title, children }) => {
            let children = render_all(children);
            view! {
                <div class="card">
                    {title.clone().map(|t| view! { <div class="card-header"><h3 class="card-title">{t}</h3></div> })}
                    <div class="card-body">{children}</div>
                </div>
            }
            .into_any()
        }

        PageElement::Section(Section { title, children }) => {
            let children = render_all(children);
            view! {
                <section class="plugin-section">
                    {title.clone().map(|t| view! { <h2 class="plugin-section-title">{t}</h2> })}
                    {children}
                </section>
            }
            .into_any()
        }

        PageElement::Table(Table { headers, rows }) => {
            let headers = headers.clone();
            let rows = rows.clone();
            view! {
                <div class="table-container">
                    <table class="table">
                        <thead>
                            <tr>{headers.into_iter().map(|h| view! { <th>{h}</th> }).collect_view()}</tr>
                        </thead>
                        <tbody>
                            {rows
                                .into_iter()
                                .map(|cells| view! {
                                    <tr>{cells.into_iter().map(|c| view! { <td>{c}</td> }).collect_view()}</tr>
                                })
                                .collect_view()}
                        </tbody>
                    </table>
                </div>
            }
            .into_any()
        }

        PageElement::Grid(Grid { columns, children }) => {
            let children = render_all(children);
            let style = format!(
                "display:grid;grid-template-columns:repeat({},minmax(0,1fr));gap:var(--space-3,12px)",
                (*columns).max(1)
            );
            view! { <div class="plugin-grid" style=style>{children}</div> }.into_any()
        }

        PageElement::Stack(Stack {
            direction,
            children,
        }) => {
            let children = render_all(children);
            let class = match direction {
                Direction::Column => "plugin-stack plugin-stack-column",
                Direction::Row => "plugin-stack plugin-stack-row",
            };
            view! { <div class=class>{children}</div> }.into_any()
        }

        PageElement::Row(Row { children }) => {
            let children = render_all(children);
            view! { <div class="plugin-stack plugin-stack-row">{children}</div> }.into_any()
        }

        PageElement::Form(Form { children }) => {
            let children = render_all(children);
            // No actions in the vocabulary, so this never submits. It is a
            // layout container that happens to be called a form.
            view! { <div class="plugin-form">{children}</div> }.into_any()
        }

        PageElement::Input(Input { label, placeholder }) => {
            let placeholder = placeholder.clone();
            view! {
                <label class="plugin-field">
                    {label.clone().map(|l| view! { <span class="plugin-field-label">{l}</span> })}
                    <input class="input" placeholder=placeholder disabled=true />
                </label>
            }
            .into_any()
        }

        PageElement::Select(Select { label, options }) => {
            let options = options.clone();
            view! {
                <label class="plugin-field">
                    {label.clone().map(|l| view! { <span class="plugin-field-label">{l}</span> })}
                    <select class="input" disabled=true>
                        {options.into_iter().map(|o| view! { <option>{o}</option> }).collect_view()}
                    </select>
                </label>
            }
            .into_any()
        }

        PageElement::Tabs(Tabs { tabs }) => {
            // Rendered stacked rather than as real tabs: switching one is an
            // interaction, and nothing here can tell the plugin about it. All
            // of the content is on the page, which is the honest fallback -
            // hiding content behind a control that does not work would lose
            // it entirely.
            let sections = tabs
                .iter()
                .map(|Tab { label, content }| {
                    let content = render_all(content);
                    view! {
                        <section class="plugin-tab">
                            <h3 class="plugin-tab-label">{label.clone()}</h3>
                            {content}
                        </section>
                    }
                })
                .collect_view();
            view! { <div class="plugin-tabs">{sections}</div> }.into_any()
        }

        PageElement::Unknown => view! {
            <div class="plugin-notice plugin-notice-warning" role="status">
                "This part of the page needs a newer version of the dashboard."
            </div>
        }
        .into_any(),
    }
}

fn render_all(children: &[PageElement]) -> Vec<AnyView> {
    children.iter().map(render).collect()
}

/// A plugin page, addressed by plugin id and path.
#[component]
pub fn PluginPageView() -> impl IntoView {
    let params = use_params_map();
    let api = expect_context::<Signal<ApiClient>>();

    let page = LocalResource::new(move || {
        let api = api.get();
        let plugin_id = params.with(|p| p.get("id").unwrap_or_default());
        let path = params.with(|p| p.get("path").unwrap_or_default());
        async move {
            if plugin_id.is_empty() || path.is_empty() {
                return Err(crate::api::ApiError::Parse("no page requested".to_string()));
            }
            api.get_plugin_page(&plugin_id, &path).await
        }
    });

    view! {
        <div class="page">
            <Suspense fallback=|| view! { <div class="loading">"Loading…"</div> }>
                {move || Suspend::new(async move {
                    match page.await {
                        Ok(element) => render(&element),
                        // The server answers 502 for a plugin that could not
                        // draw its page and 404 for one that has no such
                        // page. Neither is something a merchant can act on,
                        // so both say the same thing rather than exposing
                        // which.
                        Err(_) => view! {
                            <div class="empty-state">
                                <div class="empty-state-title">"This page is unavailable"</div>
                                <div class="empty-state-description">
                                    "It could not be loaded just now. Try again shortly."
                                </div>
                            </div>
                        }
                        .into_any(),
                    }
                })}
            </Suspense>
        </div>
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A plugin is not a trusted source of somewhere to send a merchant who
    /// is about to pay. Only a path on this origin is followed.
    #[test]
    fn only_host_relative_paths_are_followed() {
        assert_eq!(safe_href("/checkout/9f3a"), Some("/checkout/9f3a"));
        assert_eq!(safe_href("  /checkout/9f3a  "), Some("/checkout/9f3a"));

        assert_eq!(safe_href("https://evil.example/pay"), None);
        assert_eq!(safe_href("http://evil.example/pay"), None);
        assert_eq!(safe_href("javascript:alert(1)"), None);
        assert_eq!(
            safe_href("//evil.example/pay"),
            None,
            "protocol-relative has no scheme to match on and still leaves the origin"
        );
        assert_eq!(safe_href("checkout/9f3a"), None, "a bare relative path");
        assert_eq!(safe_href(""), None);
    }
}
