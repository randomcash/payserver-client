//! API Keys settings tab.

use crate::api::{
    API_KEY_UNRESTRICTED_PERMISSION, ApiClient, ApiKeyInfoWithPermissions,
    CreateApiKeyRequestWithPermissions, CreateApiKeyResponseWithPermissions,
    RotateApiKeyResponseWithPermissions, UpdateApiKeyPermissionsRequest,
    api_key_is_unrestricted, describe_api_key_permissions,
};
use leptos::prelude::*;

use super::{IconInfo, IconPlus};

/// API Keys tab.
#[component]
pub fn ApiKeysTab() -> impl IntoView {
    let api = use_context::<Signal<ApiClient>>().expect("ApiClient must be provided");

    // Track version to trigger refetches after create/revoke/permission changes
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
    let (created_key, set_created_key) =
        signal(Option::<CreateApiKeyResponseWithPermissions>::None);
    let (loading, set_loading) = signal(false);
    // Error surfaced on a failed creation — previously the handler swallowed
    // errors silently, leaving the form open with no feedback while the
    // spinner just stopped.
    let (create_error, set_create_error) = signal(Option::<String>::None);

    // Error surfaced on a failed permission change — granting unrestricted
    // access is refused server-side unless the caller's own role currently
    // grants it, so a plain user clicking "Grant unrestricted" can fail, and
    // a silent failure would leave them believing the key was widened when
    // it was not.
    let (permissions_error, set_permissions_error) = signal(Option::<String>::None);

    // Create handler
    let on_create = move |_| {
        let name = new_key_name.get();
        if name.trim().is_empty() {
            return;
        }
        let client = api.get();
        let request = CreateApiKeyRequestWithPermissions {
            name: name.trim().to_string(),
            expires_at: None,
            permissions: if new_key_unrestricted.get() {
                vec![API_KEY_UNRESTRICTED_PERMISSION.to_string()]
            } else {
                Vec::new()
            },
        };
        set_loading.set(true);
        set_create_error.set(None);
        wasm_bindgen_futures::spawn_local(async move {
            match client.create_api_key(&request).await {
                Ok(resp) => {
                    set_created_key.set(Some(resp));
                    set_show_create.set(false);
                    set_new_key_name.set(String::new());
                    set_new_key_unrestricted.set(false);
                    set_version.update(|v| *v += 1);
                }
                Err(err) => {
                    set_create_error.set(Some(format!("Failed to create key: {err}")));
                }
            }
            set_loading.set(false);
        });
    };

    // Set a key's permission scope to exactly restricted or exactly
    // unrestricted - the only two shapes the server accepts (see
    // `API_KEY_UNRESTRICTED_PERMISSION`'s doc comment for why). This is also
    // how a legacy key (`permissions: None`, inheriting its owner's role in
    // full - the incident this feature exists to close) gets narrowed for
    // the first time: "Restrict" sends `[]` regardless of the key's current
    // scope.
    let make_set_permissions_handler = move |key_id: String, unrestricted: bool| {
        let client = api.get();
        move |_| {
            let client = client.clone();
            let id = key_id.clone();
            set_permissions_error.set(None);
            wasm_bindgen_futures::spawn_local(async move {
                let request = UpdateApiKeyPermissionsRequest {
                    permissions: Some(if unrestricted {
                        vec![API_KEY_UNRESTRICTED_PERMISSION.to_string()]
                    } else {
                        Vec::new()
                    }),
                };
                match client.update_api_key_permissions(&id, &request).await {
                    Ok(_) => set_version.update(|v| *v += 1),
                    Err(err) => {
                        set_permissions_error.set(Some(format!(
                            "Failed to update this key's permissions: {err}"
                        )));
                    }
                }
            });
        }
    };

    // Revoke handler factory
    let make_revoke_handler = move |key_id: String| {
        let client = api.get();
        move |_| {
            let client = client.clone();
            let id = key_id.clone();
            wasm_bindgen_futures::spawn_local(async move {
                let _ = client.revoke_api_key(&id).await;
                set_version.update(|v| *v += 1);
            });
        }
    };

    // State for rotated key display
    let (rotated_key, set_rotated_key) =
        signal(Option::<RotateApiKeyResponseWithPermissions>::None);
    // Error surfaced on a failed rotation — previously the handler swallowed
    // errors silently, leaving the user wondering if the click had any effect.
    let (rotate_error, set_rotate_error) = signal(Option::<String>::None);

    // Rotate handler factory
    let make_rotate_handler = move |key_id: String| {
        let client = api.get();
        move |_| {
            let client = client.clone();
            let id = key_id.clone();
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
        }
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
                        <div class="form-group">
                            <label class="form-label">"Permissions"</label>
                            <p class="section-desc">
                                "Unchecked by default: a new key can authenticate but cannot "
                                "take any admin action, regardless of your own role."
                            </p>
                            <div class="api-key-permissions-list" style="border-color: var(--color-danger);">
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
                                    "Only check this if the key genuinely needs to act as you."
                                </p>
                            </div>
                        </div>
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
                        <p class="section-desc">"Can do: "{describe_api_key_permissions(&Some(key.permissions.clone()))}</p>
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

            // Permission-change error — e.g. a plain user's own key cannot
            // be granted "unrestricted" server-side, and this must not fail
            // silently.
            {move || permissions_error.get().map(|msg| view! {
                <div class="ps-card" style="margin-bottom: 16px; border-color: var(--color-danger);">
                    <div class="ps-card-body">
                        <p><strong>{msg}</strong></p>
                        <button
                            class="ps-btn ps-btn-ghost ps-btn-sm"
                            on:click=move |_| set_permissions_error.set(None)
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
                let permissions_line = format!("Can do: {}", describe_api_key_permissions(&key.permissions));
                view! {
                <div class="ps-card" style="margin-bottom: 16px; border-color: var(--color-warning);">
                    <div class="ps-card-body">
                        <p><strong>"Key rotated successfully. Copy your new key now — it will not be shown again."</strong></p>
                        <p>{grace_line}</p>
                        <code class="api-key-value" style="display: block; margin: 8px 0; padding: 8px; background: var(--color-bg-secondary); word-break: break-all;">
                            {key.key.clone()}
                        </code>
                        <p class="section-desc">{permissions_line}</p>
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
                                        {keys.into_iter().map(|key: ApiKeyInfoWithPermissions| {
                                            let (status_class, status_label) = if !key.is_active {
                                                ("badge badge-neutral".to_string(), "Revoked".to_string())
                                            } else if key.deprecated_at.is_some() {
                                                // Surface the actual expiry rather than a
                                                // static "Deprecated" — users need to know
                                                // when the grace window ends.
                                                let label = match key.deprecation_expires_at.map(|d| d.to_rfc3339()).as_deref() {
                                                    Some(exp) => format!("Deprecated — expires {exp}"),
                                                    None => "Deprecated".to_string(),
                                                };
                                                ("badge badge-warning".to_string(), label)
                                            } else {
                                                ("badge badge-success".to_string(), "Active".to_string())
                                            };
                                            let is_active = key.is_active;
                                            let is_deprecated = key.deprecated_at.is_some();
                                            let permissions_summary = describe_api_key_permissions(&key.permissions);
                                            let is_unrestricted = api_key_is_unrestricted(&key.permissions);
                                            let permissions_class = if is_unrestricted {
                                                "api-key-permissions-summary api-key-permissions-summary-unrestricted"
                                            } else {
                                                "api-key-permissions-summary"
                                            };
                                            let revoke_handler = make_revoke_handler(key.id.to_string());
                                            let rotate_handler = make_rotate_handler(key.id.to_string());
                                            let grant_unrestricted_handler =
                                                make_set_permissions_handler(key.id.to_string(), true);
                                            let restrict_handler =
                                                make_set_permissions_handler(key.id.to_string(), false);

                                            view! {
                                                <div class="api-key-item">
                                                    <div class="api-key-info">
                                                        <div class="api-key-header">
                                                            <span class="api-key-name">{key.name}</span>
                                                            <span class=status_class>{status_label}</span>
                                                        </div>
                                                        <code class="api-key-value">{key.key_prefix}</code>
                                                        <span class="api-key-created">"Created "{key.created_at.to_rfc3339()}</span>
                                                        <span class=permissions_class>"Can do: "{permissions_summary}</span>
                                                    </div>
                                                    <div class="api-key-actions">
                                                        {is_active.then(|| if is_unrestricted {
                                                            view! {
                                                                <button
                                                                    class="ps-btn ps-btn-ghost ps-btn-sm"
                                                                    on:click=restrict_handler
                                                                >
                                                                    "Restrict"
                                                                </button>
                                                            }.into_any()
                                                        } else {
                                                            view! {
                                                                <button
                                                                    class="ps-btn ps-btn-ghost ps-btn-sm"
                                                                    on:click=grant_unrestricted_handler
                                                                >
                                                                    "Grant unrestricted"
                                                                </button>
                                                            }.into_any()
                                                        })}
                                                        {(is_active && !is_deprecated).then(|| view! {
                                                            <button
                                                                class="ps-btn ps-btn-ghost ps-btn-sm"
                                                                on:click=rotate_handler
                                                            >
                                                                "Rotate"
                                                            </button>
                                                        })}
                                                        {is_active.then(|| view! {
                                                            <button
                                                                class="ps-btn ps-btn-ghost ps-btn-sm"
                                                                on:click=revoke_handler
                                                            >
                                                                "Revoke"
                                                            </button>
                                                        })}
                                                    </div>
                                                </div>
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
