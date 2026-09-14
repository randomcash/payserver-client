//! Account settings tab.

use crate::api::{ApiClient, WalletCredential};
use leptos::prelude::*;
use ui_kit::auth::wallet;
use ui_kit::use_auth;

use super::format_date;

/// Account settings tab — loads profile from `/auth/me`.
#[component]
pub fn AccountTab() -> impl IntoView {
    let api = use_context::<Signal<ApiClient>>().expect("ApiClient must be provided");
    let auth = use_auth();

    // Deleting is irreversible and cascades, so it takes two deliberate steps:
    // reveal the confirmation, then type the account's own handle back. A single
    // click - or a browser `confirm()`, which people dismiss by reflex - is not
    // enough for an action that destroys stores, wallets and API keys.
    let (confirming, set_confirming) = signal(false);
    let (typed, set_typed) = signal(String::new());
    let (deleting, set_deleting) = signal(false);
    let (delete_error, set_delete_error) = signal(Option::<String>::None);

    // "Manage wallets": which wallet is primary is a login credential, so
    // promoting one requires proving current possession of its private key,
    // not just a valid session - a hijacked session cannot produce a wallet
    // signature it never held the key for. Clicking "Make primary" opens an
    // inline confirmation (the same expand-in-place pattern as Danger Zone
    // below) rather than firing immediately; confirming drives the browser
    // wallet extension through connect -> fetch challenge -> sign -> submit.
    // `wallets_version` re-runs the list fetch after a successful swap so the
    // new primary shows immediately, the same pattern the API keys tab uses.
    let (show_wallets, set_show_wallets) = signal(false);
    let (wallets_version, set_wallets_version) = signal(0u32);
    let (confirming_wallet, set_confirming_wallet) = signal(Option::<(String, String)>::None);
    let (signing, set_signing) = signal(false);
    let (wallet_error, set_wallet_error) = signal(Option::<String>::None);

    let wallets_resource = LocalResource::new(move || {
        let api = api.get();
        let _v = wallets_version.get();
        let visible = show_wallets.get();
        async move {
            if !visible {
                return None;
            }
            Some(api.list_wallet_credentials().await)
        }
    });

    let start_confirm = move |id: String, address: String| {
        move |_| {
            set_confirming_wallet.set(Some((id.clone(), address.clone())));
            set_wallet_error.set(None);
        }
    };

    let cancel_confirm = move |_| {
        set_confirming_wallet.set(None);
        set_wallet_error.set(None);
    };

    let confirm_and_sign = move |_| {
        let Some((id, address)) = confirming_wallet.get() else {
            return;
        };
        let api = api.get();
        set_signing.set(true);
        set_wallet_error.set(None);
        leptos::task::spawn_local(async move {
            let outcome: Result<(), String> = async {
                // The extension may be connected as a different account than
                // the one being promoted - ask it to sign with this specific
                // address rather than whatever is currently active, and fail
                // clearly if the two do not match.
                let connected = wallet::connect_wallet().await.map_err(|e| e.to_string())?;
                if !connected.eq_ignore_ascii_case(&address) {
                    return Err(format!(
                        "Your wallet extension is connected as {connected}. Switch it to {address} in your extension and try again."
                    ));
                }
                let challenge = api
                    .create_wallet_reauth_challenge(&id)
                    .await
                    .map_err(|e| e.to_string())?;
                let signature = wallet::sign_message(&address, &challenge.message)
                    .await
                    .map_err(|e| e.to_string())?;
                api.set_primary_wallet_credential(&id, &signature)
                    .await
                    .map_err(|e| e.to_string())?;
                Ok(())
            }
            .await;

            match outcome {
                Ok(()) => {
                    set_wallets_version.update(|v| *v += 1);
                    set_confirming_wallet.set(None);
                }
                Err(e) => set_wallet_error.set(Some(e)),
            }
            set_signing.set(false);
        });
    };

    let user_resource = LocalResource::new(move || {
        let api = api.get();
        async move { api.get_me().await }
    });

    view! {
        <div class="settings-tab-account">
            <Suspense fallback=move || view! {
                <div style="text-align: center; padding: 3rem; color: var(--text-muted);">
                    "Loading profile..."
                </div>
            }>
                {move || user_resource.get().map(|result| match &*result {
                    Ok(user) => {
                        let email_display = user.email.clone().unwrap_or_else(|| "Not set".to_string());
                        let wallet_display = user.primary_wallet_address.clone().unwrap_or_else(|| "Not set".to_string());
                        let role_label = user.role.label().to_string();
                        let role_class = if user.role.is_admin() { "badge badge-warning" } else { "badge badge-neutral" };
                        let created = format_date(&user.created_at);
                        let last_login = user.last_login_at.as_deref().map(format_date).unwrap_or_else(|| "Never".to_string());
                        let user_id = user.id.clone();
                        // What the server will compare against: the email where
                        // there is one, else the account id. A passkey-only
                        // account has neither email nor wallet, so its id is the
                        // only handle it has.
                        let expected = user.email.clone().unwrap_or_else(|| user.id.clone());
                        // `StoredValue` so the handlers below stay `Copy`. A
                        // captured `String` would make `on_delete` `FnOnce`, and
                        // a reactive block has to be callable more than once.
                        let expected_stored = StoredValue::new(expected.clone());
                        let matches = move || {
                            expected_stored.with_value(|e| {
                                typed.get().trim().eq_ignore_ascii_case(e.trim())
                            })
                        };
                        let on_delete = move |_| {
                            if !matches() {
                                return;
                            }
                            let api = api.get();
                            let confirm = expected_stored.get_value();
                            set_deleting.set(true);
                            set_delete_error.set(None);
                            leptos::task::spawn_local(async move {
                                match api.delete_account(&confirm).await {
                                    Ok(()) => {
                                        // The session died with the account; drop
                                        // the local token so the app does not keep
                                        // retrying with it.
                                        auth.logout();
                                    }
                                    Err(e) => {
                                        // Includes the 409 naming the payments,
                                        // payouts or refunds holding the account.
                                        // Shown verbatim: "you have 3 payments" is
                                        // the answer, not a failure to report one.
                                        let _ = set_delete_error.try_set(Some(e.to_string()));
                                        let _ = set_deleting.try_set(false);
                                    }
                                }
                            });
                        };
                        let has_email = user.email.is_some();
                        let has_wallet = user.primary_wallet_address.is_some();

                        view! {
                            <div class="ps-card">
                                <div class="ps-card-header">
                                    <h3>"Profile Information"</h3>
                                    <span class=role_class>{role_label}</span>
                                </div>
                                <div class="ps-card-body">
                                    <div class="form-group">
                                        <label class="form-label">"User ID"</label>
                                        <div class="form-static">
                                            <code>{user_id}</code>
                                        </div>
                                    </div>

                                    <div class="form-group">
                                        <label class="form-label">"Email Address"</label>
                                        <div class="form-static">
                                            {if has_email {
                                                view! { <span>{email_display}</span> }.into_any()
                                            } else {
                                                view! { <span class="text-muted">"Not set (wallet-only account)"</span> }.into_any()
                                            }}
                                        </div>
                                    </div>

                                    <div class="form-group">
                                        <label class="form-label">"Wallet Address"</label>
                                        <div class="form-static">
                                            {if has_wallet {
                                                view! { <code>{wallet_display}</code> }.into_any()
                                            } else {
                                                view! { <span class="text-muted">"Not set (email-only account)"</span> }.into_any()
                                            }}
                                        </div>
                                    </div>

                                    <div class="settings-grid">
                                        <div class="form-group">
                                            <label class="form-label">"Account Created"</label>
                                            <div class="form-static">{created}</div>
                                        </div>
                                        <div class="form-group">
                                            <label class="form-label">"Last Login"</label>
                                            <div class="form-static">{last_login}</div>
                                        </div>
                                    </div>
                                </div>
                            </div>

                            <div class="ps-card">
                                <div class="ps-card-header">
                                    <h3>"Security"</h3>
                                </div>
                                <div class="ps-card-body">
                                    <p class="form-help" style="margin-bottom: 16px;">
                                        "This server uses passwordless authentication. Manage your passkeys and connected wallets to control access to your account."
                                    </p>
                                    <div class="form-actions">
                                        <button class="ps-btn ps-btn-secondary ps-btn-sm" disabled=true>
                                            "Manage passkeys"
                                        </button>
                                        <button
                                            class="ps-btn ps-btn-secondary ps-btn-sm"
                                            on:click=move |_| set_show_wallets.update(|v| *v = !*v)
                                        >
                                            {move || if show_wallets.get() { "Hide wallets" } else { "Manage wallets" }}
                                        </button>
                                    </div>

                                    {move || show_wallets.get().then(|| view! {
                                        <div class="settings-wallets" style="margin-top: 16px;">
                                            {move || wallet_error.get().map(|msg| view! {
                                                <p style="color: var(--color-error); margin-bottom: 8px;">{msg}</p>
                                            })}
                                            <Suspense fallback=move || view! { <p>"Loading wallets..."</p> }>
                                                {move || Suspend::new(async move {
                                                    match wallets_resource.await {
                                                        Some(Ok(wallets)) => {
                                                            if wallets.is_empty() {
                                                                view! { <p class="empty-state">"No wallet credentials on this account."</p> }.into_any()
                                                            } else {
                                                                view! {
                                                                    <div class="wallet-credentials-list">
                                                                        {wallets.into_iter().map(|w: WalletCredential| {
                                                                            let is_primary = w.is_primary;
                                                                            let id = w.id.clone();
                                                                            let address = w.address.clone();
                                                                            let id_for_open = id.clone();
                                                                            let id_for_disabled = id.clone();
                                                                            let id_for_panel = id.clone();
                                                                            let is_confirming_disabled = move || {
                                                                                confirming_wallet.get().as_ref().map(|(cid, _)| cid.as_str()) == Some(id_for_disabled.as_str())
                                                                            };
                                                                            let is_confirming_panel = move || {
                                                                                confirming_wallet.get().as_ref().map(|(cid, _)| cid.as_str()) == Some(id_for_panel.as_str())
                                                                            };
                                                                            view! {
                                                                                <div class="wallet-credential-item">
                                                                                    <div class="wallet-credential-info">
                                                                                        <code>{w.address.clone()}</code>
                                                                                        <span class="text-muted">{w.name.clone()}</span>
                                                                                    </div>
                                                                                    {if is_primary {
                                                                                        view! { <span class="badge badge-success">"Primary"</span> }.into_any()
                                                                                    } else {
                                                                                        view! {
                                                                                            <button
                                                                                                class="ps-btn ps-btn-ghost ps-btn-sm"
                                                                                                prop:disabled=is_confirming_disabled
                                                                                                on:click=start_confirm(id_for_open, address)
                                                                                            >
                                                                                                "Make primary"
                                                                                            </button>
                                                                                        }.into_any()
                                                                                    }}
                                                                                </div>
                                                                                {move || is_confirming_panel().then(|| view! {
                                                                                    <div class="wallet-credential-confirm" style="margin: 4px 0 12px; padding-left: 4px;">
                                                                                        <p class="form-help">
                                                                                            "This changes the wallet your account signs in with. Confirm in your browser wallet extension to prove you control it."
                                                                                        </p>
                                                                                        <div class="form-actions">
                                                                                            <button
                                                                                                class="ps-btn ps-btn-primary ps-btn-sm"
                                                                                                on:click=confirm_and_sign
                                                                                                disabled=move || signing.get()
                                                                                            >
                                                                                                {move || if signing.get() { "Confirming..." } else { "Sign & confirm" }}
                                                                                            </button>
                                                                                            <button
                                                                                                class="ps-btn ps-btn-secondary ps-btn-sm"
                                                                                                on:click=cancel_confirm
                                                                                                disabled=move || signing.get()
                                                                                            >
                                                                                                "Cancel"
                                                                                            </button>
                                                                                        </div>
                                                                                    </div>
                                                                                })}
                                                                            }
                                                                        }).collect_view()}
                                                                    </div>
                                                                }.into_any()
                                                            }
                                                        }
                                                        Some(Err(e)) => view! {
                                                            <p class="text-error">"Failed to load wallets: "{e.to_string()}</p>
                                                        }.into_any(),
                                                        None => view! { <span /> }.into_any(),
                                                    }
                                                })}
                                            </Suspense>
                                        </div>
                                    })}
                                </div>
                            </div>

                            <div class="ps-card ps-card-danger">
                                <div class="ps-card-header">
                                    <h3>"Danger Zone"</h3>
                                </div>
                                <div class="ps-card-body">
                                    <div class="danger-action">
                                        <div class="danger-action-info">
                                            <span class="danger-action-title">"Delete account"</span>
                                            <span class="danger-action-desc">
                                                "Deletes this account, its stores, wallets and API keys. \
                                                 Refused if any store has taken a payment - that history \
                                                 cannot be destroyed. Addresses already issued stay valid \
                                                 and funds sent to them remain yours."
                                            </span>
                                        </div>
                                        <button
                                            class="ps-btn ps-btn-danger ps-btn-sm"
                                            on:click=move |_| set_confirming.set(true)
                                            disabled=move || confirming.get()
                                        >
                                            "Delete account"
                                        </button>
                                    </div>

                                    {
                                        // Owned by the closure and cloned per
                                        // call: cloning the captured value
                                        // directly would move it, and a reactive
                                        // block has to be callable more than once.
                                        let expected_label = expected.clone();
                                        move || confirming.get().then(|| {
                                        let expected = expected_label.clone();
                                        view! {
                                            <div class="form-group">
                                                <label class="form-label">
                                                    "Type " <code>{expected.clone()}</code> " to confirm"
                                                </label>
                                                <input
                                                    type="text"
                                                    class="form-input"
                                                    prop:value=move || typed.get()
                                                    on:input=move |ev| set_typed.set(event_target_value(&ev))
                                                />
                                                {move || delete_error.get().map(|msg| view! {
                                                    <p style="color: var(--color-error)">{msg}</p>
                                                })}
                                                <div class="form-actions">
                                                    <button
                                                        class="ps-btn ps-btn-danger ps-btn-sm"
                                                        on:click=on_delete
                                                        disabled=move || !matches() || deleting.get()
                                                    >
                                                        {move || if deleting.get() {
                                                            "Deleting..."
                                                        } else {
                                                            "Permanently delete this account"
                                                        }}
                                                    </button>
                                                    <button
                                                        class="ps-btn ps-btn-secondary ps-btn-sm"
                                                        on:click=move |_| {
                                                            set_confirming.set(false);
                                                            set_typed.set(String::new());
                                                            set_delete_error.set(None);
                                                        }
                                                        disabled=move || deleting.get()
                                                    >
                                                        "Cancel"
                                                    </button>
                                                </div>
                                            </div>
                                        }
                                    })
                                    }
                                </div>
                            </div>
                        }.into_any()
                    }
                    Err(e) => view! {
                        <div class="ps-card">
                            <div class="ps-card-body">
                                <p class="text-error">"Failed to load profile: "{e.to_string()}</p>
                            </div>
                        </div>
                    }.into_any(),
                })}
            </Suspense>
        </div>
    }
}
