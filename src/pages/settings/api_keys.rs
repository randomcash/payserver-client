//! API Keys settings tab.

use std::collections::HashSet;

use crate::api::{
    API_KEY_GRANTABLE_STORE_ACTIONS, ApiClient, ApiKeyInfoWithPermissions,
    CreateApiKeyResponseWithPermissions, CreateKeyRefusal, RotateApiKeyResponseWithPermissions,
    api_key_is_unrestricted, describe_api_key_permissions, plan_create_api_key_request,
};
use leptos::prelude::*;

use super::{IconInfo, IconPlus};

const NO_PERMISSIONS_MESSAGE: &str =
    "Grant at least one permission: a key with none cannot do anything.";

/// Tick or untick one action in the create form's selection.
fn set_action(actions: &mut HashSet<String>, policy: String, checked: bool) {
    if checked {
        actions.insert(policy);
    } else {
        actions.remove(&policy);
    }
}

/// What pressing "Create key" did, decided from the form's state alone so the
/// whole path from ticked boxes to the request on the wire runs under
/// `cargo test`.
enum CreateOutcome {
    /// Blank name: nothing is sent and nothing is shown.
    Ignored,
    /// Refused before the server was asked; the message is for the user.
    Refused(String),
    Created(CreateApiKeyResponseWithPermissions),
    Failed(String),
}

async fn submit_create_key(
    client: &ApiClient,
    name: &str,
    unrestricted: bool,
    actions: &HashSet<String>,
) -> CreateOutcome {
    let request = match plan_create_api_key_request(name, unrestricted, actions) {
        Ok(request) => request,
        Err(CreateKeyRefusal::BlankName) => return CreateOutcome::Ignored,
        Err(CreateKeyRefusal::NoPermissions) => {
            return CreateOutcome::Refused(NO_PERMISSIONS_MESSAGE.to_string());
        }
    };
    match client.create_api_key(&request).await {
        Ok(resp) => CreateOutcome::Created(resp),
        Err(err) => CreateOutcome::Failed(format!("Failed to create key: {err}")),
    }
}

/// The scope line shown for a key: on the list, and on a just-created or
/// just-rotated key. Unrestricted keys get their own class so they read as
/// the dangerous ones.
#[component]
fn ApiKeyScope(permissions: Option<Vec<String>>) -> impl IntoView {
    let class = if api_key_is_unrestricted(&permissions) {
        "api-key-permissions-summary api-key-permissions-summary-unrestricted"
    } else {
        "api-key-permissions-summary"
    };
    view! {
        <span class=class>
            "Can do: "
            {describe_api_key_permissions(&permissions)}
        </span>
    }
}

/// The create form's permission picker. "Unrestricted" overrides the
/// individual actions, so their boxes are disabled while it is ticked.
#[component]
fn PermissionChecklist(
    actions: ReadSignal<HashSet<String>>,
    set_actions: WriteSignal<HashSet<String>>,
    unrestricted: ReadSignal<bool>,
    set_unrestricted: WriteSignal<bool>,
) -> impl IntoView {
    let (new_key_store_actions, set_new_key_store_actions) = (actions, set_actions);
    let (new_key_unrestricted, set_new_key_unrestricted) = (unrestricted, set_unrestricted);
    view! {
        <div class="form-group">
            <label class="form-label">"Permissions"</label>
            <p class="section-desc">
                "Unchecked by default: a new key can authenticate but cannot "
                "do anything else until you grant it something below."
            </p>
            <div class="api-key-permissions-list">
                {API_KEY_GRANTABLE_STORE_ACTIONS.iter().map(|(policy, label)| {
                    let policy_for_checked = policy.to_string();
                    let policy_for_change = policy.to_string();
                    view! {
                        <label class="api-key-permission-item">
                            <input
                                type="checkbox"
                                prop:checked=move || new_key_store_actions.get().contains(&policy_for_checked)
                                prop:disabled=move || new_key_unrestricted.get()
                                on:change=move |ev| {
                                    let checked = event_target_checked(&ev);
                                    let policy = policy_for_change.clone();
                                    set_new_key_store_actions
                                        .update(|actions| set_action(actions, policy, checked));
                                }
                            />
                            {*label}
                        </label>
                    }
                }).collect_view()}
                <p class="section-desc">
                    "Granted on every store you can reach. Each action here is "
                    "checked individually by the server, so selecting one really "
                    "does grant only that one."
                </p>
            </div>
            <div class="api-key-permissions-list" style="border-color: var(--color-danger); margin-top: 8px;">
                <label class="api-key-permission-item">
                    <input
                        type="checkbox"
                        prop:checked=move || new_key_unrestricted.get()
                        on:change=move |ev| set_new_key_unrestricted.set(event_target_checked(&ev))
                    />
                    <strong>"Unrestricted (full account access)"</strong>
                </label>
                <p class="section-desc">
                    "Equivalent to your own full role, including installing plugins "
                    "- which runs arbitrary SQL and arbitrary code on the server. "
                    "Only check this if the key genuinely needs to act as you. "
                    "Overrides the actions above."
                </p>
            </div>
        </div>
    }
}

/// One key in the list: status, prefix, scope, and the actions its state
/// allows. Rotate and Revoke hand back the key's id.
#[component]
fn ApiKeyRow(
    key: ApiKeyInfoWithPermissions,
    on_rotate: Callback<String>,
    on_revoke: Callback<String>,
) -> impl IntoView {
    let (status_class, status_label) = if !key.is_active {
        ("badge badge-neutral".to_string(), "Revoked".to_string())
    } else if key.deprecated_at.is_some() {
        // Surface the actual expiry rather than a
        // static "Deprecated" — users need to know
        // when the grace window ends.
        let label = match key
            .deprecation_expires_at
            .map(|d| d.to_rfc3339())
            .as_deref()
        {
            Some(exp) => format!("Deprecated — expires {exp}"),
            None => "Deprecated".to_string(),
        };
        ("badge badge-warning".to_string(), label)
    } else {
        ("badge badge-success".to_string(), "Active".to_string())
    };
    let is_active = key.is_active;
    let is_deprecated = key.deprecated_at.is_some();
    let id = key.id.to_string();

    view! {
        <div class="api-key-item">
            <div class="api-key-info">
                <div class="api-key-header">
                    <span class="api-key-name">{key.name}</span>
                    <span class=status_class>{status_label}</span>
                </div>
                <code class="api-key-value">{key.key_prefix}</code>
                <span class="api-key-created">"Created "{key.created_at.to_rfc3339()}</span>
                <ApiKeyScope permissions=key.permissions />
            </div>
            <div class="api-key-actions">
                {(is_active && !is_deprecated).then(|| view! {
                    <button
                        class="ps-btn ps-btn-ghost ps-btn-sm"
                        on:click={let id = id.clone(); move |_| on_rotate.run(id.clone())}
                    >
                        "Rotate"
                    </button>
                })}
                {is_active.then(|| view! {
                    <button
                        class="ps-btn ps-btn-ghost ps-btn-sm"
                        on:click={let id = id.clone(); move |_| on_revoke.run(id.clone())}
                    >
                        "Revoke"
                    </button>
                })}
            </div>
        </div>
    }
}

/// API Keys tab.
#[component]
pub fn ApiKeysTab() -> impl IntoView {
    let api = use_context::<Signal<ApiClient>>().expect("ApiClient must be provided");

    // Track version to trigger refetches after create/revoke/rotate
    let (version, set_version) = signal(0u32);

    // Load API keys from backend
    let keys_resource = LocalResource::new(move || {
        let client = api.get();
        let _v = version.get();
        async move { client.list_api_keys().await.ok() }
    });

    // State for create form
    let (show_create, set_show_create) = signal(false);
    let (new_key_name, set_new_key_name) = signal(String::new());
    let (new_key_unrestricted, set_new_key_unrestricted) = signal(false);
    let (new_key_store_actions, set_new_key_store_actions) = signal(HashSet::<String>::new());
    let (created_key, set_created_key) =
        signal(Option::<CreateApiKeyResponseWithPermissions>::None);
    let (loading, set_loading) = signal(false);
    // Error surfaced on a failed creation — previously the handler swallowed
    // errors silently, leaving the form open with no feedback while the
    // spinner just stopped.
    let (create_error, set_create_error) = signal(Option::<String>::None);

    // Create handler
    let on_create = move |_| {
        let client = api.get();
        let name = new_key_name.get();
        let unrestricted = new_key_unrestricted.get();
        let actions = new_key_store_actions.get();
        set_create_error.set(None);
        wasm_bindgen_futures::spawn_local(async move {
            set_loading.set(true);
            match submit_create_key(&client, &name, unrestricted, &actions).await {
                CreateOutcome::Ignored => {}
                CreateOutcome::Refused(msg) | CreateOutcome::Failed(msg) => {
                    set_create_error.set(Some(msg));
                }
                CreateOutcome::Created(resp) => {
                    set_created_key.set(Some(resp));
                    set_show_create.set(false);
                    set_new_key_name.set(String::new());
                    set_new_key_unrestricted.set(false);
                    set_new_key_store_actions.set(HashSet::new());
                    set_version.update(|v| *v += 1);
                }
            }
            set_loading.set(false);
        });
    };

    // Revoke handler factory
    let make_revoke_handler = move |id: String| {
        let client = api.get();
        wasm_bindgen_futures::spawn_local(async move {
            let _ = client.revoke_api_key(&id).await;
            set_version.update(|v| *v += 1);
        });
    };

    // State for rotated key display
    let (rotated_key, set_rotated_key) =
        signal(Option::<RotateApiKeyResponseWithPermissions>::None);
    // Error surfaced on a failed rotation — previously the handler swallowed
    // errors silently, leaving the user wondering if the click had any effect.
    let (rotate_error, set_rotate_error) = signal(Option::<String>::None);

    // Rotate handler factory
    let make_rotate_handler = move |id: String| {
        let client = api.get();
        set_rotate_error.set(None);
        wasm_bindgen_futures::spawn_local(async move {
            match client.rotate_api_key(&id).await {
                Ok(resp) => {
                    set_rotated_key.set(Some(resp));
                    set_version.update(|v| *v += 1);
                }
                Err(err) => {
                    set_rotate_error.set(Some(format!("Failed to rotate key: {err}")));
                }
            }
        });
    };

    view! {
        <div class="settings-tab-api-keys">
            <div class="section-header">
                <div>
                    <h3 class="section-title">"API Keys"</h3>
                    <p class="section-desc">"Manage API keys for programmatic access"</p>
                </div>
                <button
                    class="ps-btn ps-btn-primary ps-btn-sm"
                    on:click=move |_| set_show_create.set(true)
                >
                    <IconPlus />
                    "Create API key"
                </button>
            </div>

            // Create form
            {move || show_create.get().then(|| view! {
                <div class="ps-card" style="margin-bottom: 16px;">
                    <div class="ps-card-header">
                        <h3>"Create new API key"</h3>
                    </div>
                    <div class="ps-card-body">
                        <div class="form-group">
                            <label class="form-label">"Key name"</label>
                            <input
                                type="text"
                                class="form-input"
                                placeholder="e.g., Production Key"
                                prop:value=move || new_key_name.get()
                                on:input=move |ev| set_new_key_name.set(event_target_value(&ev))
                            />
                        </div>
                        <PermissionChecklist
                            actions=new_key_store_actions
                            set_actions=set_new_key_store_actions
                            unrestricted=new_key_unrestricted
                            set_unrestricted=set_new_key_unrestricted
                        />
                        <div class="form-actions">
                            <button
                                class="ps-btn ps-btn-primary ps-btn-sm"
                                prop:disabled=move || loading.get()
                                on:click=on_create
                            >
                                {move || if loading.get() { "Creating..." } else { "Create key" }}
                            </button>
                            <button
                                class="ps-btn ps-btn-ghost ps-btn-sm"
                                on:click=move |_| set_show_create.set(false)
                            >
                                "Cancel"
                            </button>
                        </div>
                    </div>
                </div>
            })}

            // Creation error — a failed click must not just close the form
            // silently.
            {move || create_error.get().map(|msg| view! {
                <div class="ps-card" style="margin-bottom: 16px; border-color: var(--color-danger);">
                    <div class="ps-card-body">
                        <p><strong>{msg}</strong></p>
                        <button
                            class="ps-btn ps-btn-ghost ps-btn-sm"
                            on:click=move |_| set_create_error.set(None)
                        >
                            "Dismiss"
                        </button>
                    </div>
                </div>
            })}

            // Show newly created key (plaintext shown once)
            {move || created_key.get().map(|key| view! {
                <div class="ps-card" style="margin-bottom: 16px; border-color: var(--color-success);">
                    <div class="ps-card-body">
                        <p><strong>"Your new API key has been created. Copy it now — it will not be shown again."</strong></p>
                        <code class="api-key-value" style="display: block; margin: 8px 0; padding: 8px; background: var(--color-bg-secondary); word-break: break-all;">
                            {key.key.clone()}
                        </code>
                        <p class="section-desc"><ApiKeyScope permissions=key.permissions.clone() /></p>
                        <button
                            class="ps-btn ps-btn-ghost ps-btn-sm"
                            on:click=move |_| set_created_key.set(None)
                        >
                            "Dismiss"
                        </button>
                    </div>
                </div>
            })}

            // Rotation error — user must know when a rotate click failed.
            {move || rotate_error.get().map(|msg| view! {
                <div class="ps-card" style="margin-bottom: 16px; border-color: var(--color-danger);">
                    <div class="ps-card-body">
                        <p><strong>{msg}</strong></p>
                        <button
                            class="ps-btn ps-btn-ghost ps-btn-sm"
                            on:click=move |_| set_rotate_error.set(None)
                        >
                            "Dismiss"
                        </button>
                    </div>
                </div>
            })}

            // Show rotated key (plaintext shown once)
            {move || rotated_key.get().map(|key| {
                // Always sent by the server, so there is no "unknown" branch to
                // render - the client used to model this as optional and carry
                // a fallback that could never fire.
                let grace_line = format!(
                    "The old key remains valid until {}.",
                    key.old_key_grace_expires_at.to_rfc3339()
                );
                view! {
                <div class="ps-card" style="margin-bottom: 16px; border-color: var(--color-warning);">
                    <div class="ps-card-body">
                        <p><strong>"Key rotated successfully. Copy your new key now — it will not be shown again."</strong></p>
                        <p>{grace_line}</p>
                        <code class="api-key-value" style="display: block; margin: 8px 0; padding: 8px; background: var(--color-bg-secondary); word-break: break-all;">
                            {key.key.clone()}
                        </code>
                        <p class="section-desc"><ApiKeyScope permissions=key.permissions.clone() /></p>
                        <button
                            class="ps-btn ps-btn-ghost ps-btn-sm"
                            on:click=move |_| set_rotated_key.set(None)
                        >
                            "Dismiss"
                        </button>
                    </div>
                </div>
            }})}

            // API keys list
            <Suspense fallback=move || view! { <p>"Loading API keys..."</p> }>
                {move || Suspend::new(async move {
                    match keys_resource.await {
                        Some(resp) => {
                            let keys = resp.keys;
                            if keys.is_empty() {
                                view! { <p class="empty-state">"No API keys yet. Create one to get started."</p> }.into_any()
                            } else {
                                view! {
                                    <div class="api-keys-list">
                                        {keys.into_iter().map(|key| {
                                            view! {
                                                <ApiKeyRow
                                                    key
                                                    on_rotate=Callback::new(make_rotate_handler)
                                                    on_revoke=Callback::new(make_revoke_handler)
                                                />
                                            }
                                        }).collect_view()}
                                    </div>
                                }.into_any()
                            }
                        }
                        None => view! { <p class="error">"Failed to load API keys"</p> }.into_any(),
                    }
                })}
            </Suspense>

            <div class="settings-info">
                <IconInfo />
                <div>
                    <p><strong>"Keep your API keys secure"</strong></p>
                    <p>"Never share your API keys publicly or commit them to version control. Use environment variables in production."</p>
                </div>
            </div>
        </div>
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use leptos::prelude::RenderHtml;

    fn scope_html(permissions: Option<Vec<String>>) -> String {
        view! { <ApiKeyScope permissions /> }.to_html()
    }

    fn checklist_html() -> String {
        let (actions, set_actions) = signal(HashSet::<String>::new());
        let (unrestricted, set_unrestricted) = signal(false);
        view! { <PermissionChecklist actions set_actions unrestricted set_unrestricted /> }
            .to_html()
    }

    #[test]
    fn a_narrowed_key_lists_only_what_it_was_granted() {
        let html = scope_html(Some(vec!["ethpay.store.cancreateinvoice".to_string()]));
        assert!(html.contains("Can do: "), "{html}");
        assert!(html.contains("Create invoices"), "{html}");
        assert!(!html.contains("Modify store settings"), "{html}");
        assert!(!html.contains("unrestricted"), "{html}");
    }

    #[test]
    fn an_unrestricted_key_is_marked_as_the_dangerous_one() {
        for permissions in [None, Some(vec!["unrestricted".to_string()])] {
            let html = scope_html(permissions);
            assert!(
                html.contains("api-key-permissions-summary-unrestricted"),
                "{html}"
            );
            assert!(html.contains("Full access"), "{html}");
        }
    }

    #[test]
    fn a_key_with_nothing_granted_says_so() {
        assert!(scope_html(Some(vec![])).contains("No permissions granted"));
    }

    #[test]
    fn the_create_form_offers_every_grantable_action_and_unrestricted() {
        let html = checklist_html();
        for (_, label) in API_KEY_GRANTABLE_STORE_ACTIONS {
            assert!(html.contains(label), "missing {label}: {html}");
        }
        // One box per action, plus the unrestricted one.
        assert_eq!(
            html.matches(r#"type="checkbox""#).count(),
            API_KEY_GRANTABLE_STORE_ACTIONS.len() + 1,
            "{html}"
        );
        assert!(html.contains("Unrestricted"), "{html}");
        // The picker must not offer server-level permissions the server does
        // not enforce individually.
        assert!(!html.contains("ethpay.server."), "{html}");
    }

    // The create flow, driven from the form's state through the request on the
    // wire and back into the markup the list and the new-key card render.
    // The transport stands in for the server: it records the request and
    // answers the way the server does, echoing the permissions it was sent.
    mod flow {
        use super::*;
        use crate::api::client::{RequestSpec, TestTransport};
        use std::sync::{Arc, Mutex};

        fn block_on<F: std::future::Future>(fut: F) -> F::Output {
            let mut fut = std::pin::pin!(fut);
            let mut cx = std::task::Context::from_waker(std::task::Waker::noop());
            loop {
                if let std::task::Poll::Ready(value) = fut.as_mut().poll(&mut cx) {
                    return value;
                }
            }
        }

        const KEY_ID: &str = "0b1f2f0e-5a54-4b43-8d6a-1c3e8f3d2a10";

        fn server_double(status: Option<u16>) -> (ApiClient, Arc<Mutex<Vec<RequestSpec>>>) {
            let seen = Arc::new(Mutex::new(Vec::new()));
            let log = seen.clone();
            let transport: TestTransport = Arc::new(move |spec| {
                log.lock().unwrap().push(spec.clone());
                if let Some(status) = status {
                    return Err(crate::api::ApiError::Http {
                        status,
                        message: "refused".to_string(),
                    });
                }
                let permissions = spec.body.as_ref().map(|b| b["permissions"].clone());
                Ok(serde_json::json!({
                    "id": KEY_ID,
                    "name": "ci",
                    "key_prefix": "rcs_abcd",
                    "is_active": true,
                    "created_at": "2026-01-01T00:00:00Z",
                    "expires_at": null,
                    "key": "rcs_abcd_secret",
                    "permissions": permissions,
                }))
            });
            (ApiClient::with_test_transport("", transport), seen)
        }

        fn ticked(policies: &[&str]) -> HashSet<String> {
            let mut actions = HashSet::new();
            for policy in policies {
                set_action(&mut actions, policy.to_string(), true);
            }
            actions
        }

        fn list_row_html(permissions: serde_json::Value) -> String {
            let key: ApiKeyInfoWithPermissions = serde_json::from_value(serde_json::json!({
                "id": KEY_ID,
                "name": "ci",
                "key_prefix": "rcs_abcd",
                "is_active": true,
                "created_at": "2026-01-01T00:00:00Z",
                "last_used_at": null,
                "expires_at": null,
                "rate_limit_rpm": null,
                "deprecated_at": null,
                "deprecation_expires_at": null,
                "permissions": permissions,
            }))
            .unwrap();
            view! {
                <ApiKeyRow
                    key
                    on_rotate=Callback::new(|_| {})
                    on_revoke=Callback::new(|_| {})
                />
            }
            .to_html()
        }

        #[test]
        fn a_narrowed_key_is_sent_with_only_the_ticked_action_and_listed_with_that_scope() {
            let (client, seen) = server_double(None);
            let actions = ticked(&["ethpay.store.cancreateinvoice"]);

            let CreateOutcome::Created(created) =
                block_on(submit_create_key(&client, "ci", false, &actions))
            else {
                panic!("expected the key to be created");
            };

            let sent = seen.lock().unwrap().clone();
            assert_eq!(sent.len(), 1);
            assert_eq!(sent[0].method, "POST");
            assert_eq!(sent[0].path, "/api/users/api-keys");
            assert_eq!(
                sent[0].body.as_ref().unwrap()["permissions"],
                serde_json::json!(["ethpay.store.cancreateinvoice"])
            );

            // The new-key card, then the list, both show the narrowed scope.
            let card = scope_html(created.permissions.clone());
            assert!(card.contains("Create invoices"), "{card}");
            assert!(!card.contains("Full access"), "{card}");
            let row = list_row_html(serde_json::json!(created.permissions));
            assert!(row.contains("Create invoices"), "{row}");
            assert!(!row.contains("Modify store settings"), "{row}");
            assert!(!row.contains("Full access"), "{row}");
        }

        #[test]
        fn unticking_an_action_removes_it_from_the_request() {
            let (client, seen) = server_double(None);
            let mut actions = ticked(&[
                "ethpay.store.cancreateinvoice",
                "ethpay.store.canviewinvoices",
            ]);
            set_action(
                &mut actions,
                "ethpay.store.canviewinvoices".to_string(),
                false,
            );

            assert!(matches!(
                block_on(submit_create_key(&client, "ci", false, &actions)),
                CreateOutcome::Created(_)
            ));
            assert_eq!(
                seen.lock().unwrap()[0].body.as_ref().unwrap()["permissions"],
                serde_json::json!(["ethpay.store.cancreateinvoice"])
            );
        }

        #[test]
        fn unrestricted_overrides_the_ticked_actions_on_the_wire() {
            let (client, seen) = server_double(None);
            let actions = ticked(&["ethpay.store.cancreateinvoice"]);

            assert!(matches!(
                block_on(submit_create_key(&client, "ci", true, &actions)),
                CreateOutcome::Created(_)
            ));
            assert_eq!(
                seen.lock().unwrap()[0].body.as_ref().unwrap()["permissions"],
                serde_json::json!(["unrestricted"])
            );
        }

        #[test]
        fn a_key_with_nothing_granted_is_refused_without_asking_the_server() {
            let (client, seen) = server_double(None);

            let CreateOutcome::Refused(msg) =
                block_on(submit_create_key(&client, "ci", false, &HashSet::new()))
            else {
                panic!("expected a refusal");
            };
            assert_eq!(msg, NO_PERMISSIONS_MESSAGE);
            assert!(seen.lock().unwrap().is_empty());
        }

        #[test]
        fn a_blank_name_sends_nothing() {
            let (client, seen) = server_double(None);
            let actions = ticked(&["ethpay.store.cancreateinvoice"]);

            assert!(matches!(
                block_on(submit_create_key(&client, "   ", false, &actions)),
                CreateOutcome::Ignored
            ));
            assert!(seen.lock().unwrap().is_empty());
        }

        #[test]
        fn a_server_refusal_is_surfaced_not_swallowed() {
            let (client, _) = server_double(Some(403));
            let actions = ticked(&["ethpay.store.cancreateinvoice"]);

            let CreateOutcome::Failed(msg) =
                block_on(submit_create_key(&client, "ci", false, &actions))
            else {
                panic!("expected a failure");
            };
            assert!(msg.contains("Failed to create key"), "{msg}");
            assert!(msg.contains("403"), "{msg}");
        }

        #[test]
        fn a_legacy_key_lists_as_full_access_and_an_active_key_offers_rotate_and_revoke() {
            let row = list_row_html(serde_json::Value::Null);
            assert!(row.contains("Full access"), "{row}");
            assert!(row.contains("Rotate"), "{row}");
            assert!(row.contains("Revoke"), "{row}");
        }
    }
}
