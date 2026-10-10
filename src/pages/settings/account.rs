//! Account settings tab.

use crate::api::ApiClient;
use leptos::prelude::*;
use ui_kit::auth::session::{get_device_id, get_device_name};
use ui_kit::auth::types::{CompletePasskeyLoginRequest, CompleteWalletLoginRequest, DeviceType};
use ui_kit::auth::wallet::sign_message;
use ui_kit::auth::webauthn::get_credential;
use ui_kit::hooks::use_api::ApiClient as ReauthApiClient;
use ui_kit::{PasskeyAuthForm, PasskeyState, WalletConnectButton, use_auth};

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

    // Bumped after a successful email change/removal so the profile card
    // reflects the new address without a full page reload. `LocalResource`
    // has no `refetch()`, but its source closure re-runs whenever a signal
    // read inside it changes - reading `refresh` here is what makes that
    // happen on demand rather than only once.
    let (refresh, set_refresh) = signal(0u32);
    let on_email_changed = Callback::new(move |()| set_refresh.update(|n| *n += 1));

    let user_resource = LocalResource::new(move || {
        let api = api.get();
        refresh.get();
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
                                        <EmailSection
                                            api=api
                                            current_email=user.email.clone()
                                            has_wallet=has_wallet
                                            on_changed=on_email_changed
                                        />
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
                                        <button class="ps-btn ps-btn-secondary ps-btn-sm" disabled=true>
                                            "Manage wallets"
                                        </button>
                                    </div>
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

// =============================================================================
// Email change
//
// SENSITIVE: this drives account-recovery-affecting endpoints. Set, change and
// remove all require a session from a *freshly completed* passkey or wallet
// login (`ReauthGate`), not the page's ambient session - a hijacked long-lived
// session must not be able to swap the account's recovery address. The
// server enforces this independently (`FreshlyAuthenticatedUser` in
// ethpayserver); this UI exists to make satisfying that check possible at all.
// =============================================================================

#[derive(Clone, Copy, PartialEq, Eq)]
enum EmailAction {
    Change,
    Remove,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum EmailStep {
    Idle,
    EnterNewEmail,
    Reauthenticating,
    AwaitingCode,
}

/// The account's email, with the change/remove/verify flow behind it.
///
/// A fresh instance is mounted every time the parent's `user_resource`
/// refetches (see `AccountTab`), which is what resets this back to `Idle`
/// with the new address after a successful change - there is no explicit
/// "reset" path to maintain here.
#[component]
fn EmailSection(
    api: Signal<ApiClient>,
    current_email: Option<String>,
    has_wallet: bool,
    on_changed: Callback<()>,
) -> impl IntoView {
    let (step, set_step) = signal(EmailStep::Idle);
    let (action, set_action) = signal(Option::<EmailAction>::None);
    let (new_email, set_new_email) = signal(String::new());
    let (code, set_code) = signal(String::new());
    let (busy, set_busy) = signal(false);
    let (error, set_error) = signal(Option::<String>::None);
    let (notice, set_notice) = signal(Option::<String>::None);

    let has_email = current_email.is_some();
    let email_display = current_email.unwrap_or_else(|| "Not set".to_string());
    // The address to send to `request_email_change` once re-authentication
    // succeeds. A `StoredValue` rather than reading `new_email` again inside
    // `on_reauth_success`: the text input is gone by then (the view has
    // switched to `Reauthenticating`), and nothing keeps it from changing
    // underneath a callback that captured the signal instead of the value.
    let pending_new_email = StoredValue::new(String::new());

    let start_change = move |_| {
        set_error.set(None);
        set_notice.set(None);
        set_new_email.set(String::new());
        set_step.set(EmailStep::EnterNewEmail);
    };

    let continue_to_reauth = move |_| {
        let value = new_email.get().trim().to_string();
        if value.is_empty() {
            set_error.set(Some("Enter an email address.".to_string()));
            return;
        }
        pending_new_email.set_value(value);
        set_action.set(Some(EmailAction::Change));
        set_error.set(None);
        set_step.set(EmailStep::Reauthenticating);
    };

    let start_remove = move |_| {
        set_error.set(None);
        set_notice.set(None);
        set_action.set(Some(EmailAction::Remove));
        set_step.set(EmailStep::Reauthenticating);
    };

    let cancel = move |_| {
        set_step.set(EmailStep::Idle);
        set_action.set(None);
        set_error.set(None);
    };

    let on_reauth_success = Callback::new(move |fresh_token: String| {
        // A one-off client carrying the just-proved-fresh session, separate
        // from the page's ambient `api` - the ambient session may be hours
        // old, which is exactly what `FreshlyAuthenticatedUser` refuses.
        let fresh_client = ApiClient::new("").with_token(Some(fresh_token));
        set_busy.set(true);
        set_error.set(None);
        match action.get_untracked() {
            Some(EmailAction::Change) => {
                let target = pending_new_email.get_value();
                leptos::task::spawn_local(async move {
                    match fresh_client.request_email_change(&target).await {
                        Ok(()) => {
                            set_busy.set(false);
                            set_notice.set(Some(format!(
                                "We sent a verification code to {target}. Enter it below to confirm."
                            )));
                            set_step.set(EmailStep::AwaitingCode);
                        }
                        Err(e) => {
                            set_busy.set(false);
                            set_error.set(Some(e.to_string()));
                            set_step.set(EmailStep::Idle);
                        }
                    }
                });
            }
            Some(EmailAction::Remove) => {
                leptos::task::spawn_local(async move {
                    match fresh_client.remove_email().await {
                        Ok(()) => {
                            set_busy.set(false);
                            set_notice.set(Some("Email removed.".to_string()));
                            set_step.set(EmailStep::Idle);
                            on_changed.run(());
                        }
                        Err(e) => {
                            set_busy.set(false);
                            set_error.set(Some(e.to_string()));
                            set_step.set(EmailStep::Idle);
                        }
                    }
                });
            }
            None => {
                // Unreachable today - both entry points set `action` before
                // transitioning to `Reauthenticating` - but a re-auth success
                // with no pending action is a bug, not a no-op: surface it
                // rather than silently dropping a completed passkey/wallet
                // challenge.
                set_busy.set(false);
                set_error.set(Some("Something went wrong - please try again.".to_string()));
                set_step.set(EmailStep::Idle);
            }
        }
    });

    let on_reauth_cancel = Callback::new(move |()| {
        set_step.set(EmailStep::Idle);
        set_action.set(None);
    });

    let confirm_code = move |_| {
        let value = code.get().trim().to_string();
        if value.is_empty() {
            set_error.set(Some("Enter the verification code.".to_string()));
            return;
        }
        let api = api.get();
        set_busy.set(true);
        set_error.set(None);
        leptos::task::spawn_local(async move {
            match api.confirm_email_change(&value).await {
                Ok(()) => {
                    set_busy.set(false);
                    set_notice.set(Some("Email updated.".to_string()));
                    set_step.set(EmailStep::Idle);
                    set_code.set(String::new());
                    on_changed.run(());
                }
                Err(e) => {
                    set_busy.set(false);
                    set_error.set(Some(e.to_string()));
                }
            }
        });
    };

    view! {
        <div class="email-section">
            {move || match step.get() {
                EmailStep::Idle => view! {
                    <div>
                        <div class="form-static">
                            {if has_email {
                                view! { <span>{email_display.clone()}</span> }.into_any()
                            } else {
                                view! { <span class="text-muted">"Not set (wallet-only account)"</span> }.into_any()
                            }}
                        </div>
                        {move || notice.get().map(|n| view! { <p style="color: var(--color-success)">{n}</p> })}
                        {move || error.get().map(|e| view! { <p style="color: var(--color-error)">{e}</p> })}
                        <div class="form-actions">
                            <button class="ps-btn ps-btn-secondary ps-btn-sm" on:click=start_change>
                                {if has_email { "Change email" } else { "Add email" }}
                            </button>
                            {has_email.then(|| view! {
                                <button
                                    class="ps-btn ps-btn-secondary ps-btn-sm"
                                    on:click=start_remove
                                    disabled=!has_wallet
                                    title=(!has_wallet).then(|| "Add a wallet first - removing your only recovery handle is refused.".to_string())
                                >
                                    "Remove email"
                                </button>
                            })}
                        </div>
                    </div>
                }.into_any(),

                EmailStep::EnterNewEmail => view! {
                    <div class="form-group">
                        <input
                            type="email"
                            class="form-input"
                            placeholder="new@example.com"
                            prop:value=move || new_email.get()
                            on:input=move |ev| set_new_email.set(event_target_value(&ev))
                        />
                        {move || error.get().map(|e| view! { <p style="color: var(--color-error)">{e}</p> })}
                        <div class="form-actions">
                            <button class="ps-btn ps-btn-primary ps-btn-sm" on:click=continue_to_reauth>
                                "Continue"
                            </button>
                            <button class="ps-btn ps-btn-secondary ps-btn-sm" on:click=cancel>
                                "Cancel"
                            </button>
                        </div>
                    </div>
                }.into_any(),

                EmailStep::Reauthenticating => view! {
                    <ReauthGate on_success=on_reauth_success on_cancel=on_reauth_cancel />
                }.into_any(),

                EmailStep::AwaitingCode => view! {
                    <div class="form-group">
                        {move || notice.get().map(|n| view! { <p class="form-help">{n}</p> })}
                        <input
                            type="text"
                            class="form-input"
                            placeholder="Verification code"
                            prop:value=move || code.get()
                            on:input=move |ev| set_code.set(event_target_value(&ev))
                        />
                        {move || error.get().map(|e| view! { <p style="color: var(--color-error)">{e}</p> })}
                        <div class="form-actions">
                            <button
                                class="ps-btn ps-btn-primary ps-btn-sm"
                                on:click=confirm_code
                                disabled=move || busy.get()
                            >
                                {move || if busy.get() { "Confirming..." } else { "Confirm" }}
                            </button>
                            <button
                                class="ps-btn ps-btn-secondary ps-btn-sm"
                                on:click=cancel
                                disabled=move || busy.get()
                            >
                                "Cancel"
                            </button>
                        </div>
                    </div>
                }.into_any(),
            }}
        </div>
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum ReauthTab {
    Wallet,
    Passkey,
}

/// A fresh passkey-or-wallet login, run inline without disturbing the page's
/// ambient session.
///
/// Mirrors the ceremony in `ui-kit`'s `LoginPage` (start -> sign/get credential
/// -> complete) but never calls `auth.save_login` or navigates: the point is
/// to hand the resulting session id back to the caller as proof of a fresh
/// assertion, not to log the browser into it.
#[component]
fn ReauthGate(on_success: Callback<String>, on_cancel: Callback<()>) -> impl IntoView {
    let (tab, set_tab) = signal(ReauthTab::Wallet);
    let (error, set_error) = signal(Option::<String>::None);
    let (passkey_state, set_passkey_state) = signal(PasskeyState::Ready);
    let (busy, set_busy) = signal(false);

    // A standalone client, not the page's context one: that one carries the
    // ambient (possibly hours-old) session, and this ceremony must not touch
    // it either way in or out.
    let api = StoredValue::new(ReauthApiClient::new("/api"));

    let on_wallet_connect = Callback::new(move |address: String| {
        let api = api.get_value();
        set_busy.set(true);
        set_error.set(None);

        leptos::task::spawn_local(async move {
            let challenge = match api.start_wallet_login(&address).await {
                Ok(r) => r,
                Err(e) => {
                    set_error.set(Some(format!("Could not start sign-in: {e}")));
                    set_busy.set(false);
                    return;
                }
            };

            let signature = match sign_message(&address, &challenge.challenge_message).await {
                Ok(sig) => sig,
                Err(e) => {
                    set_error.set(Some(format!("Could not sign the challenge: {e}")));
                    set_busy.set(false);
                    return;
                }
            };

            let complete_request = CompleteWalletLoginRequest {
                user_id: challenge.user_id,
                address: address.clone(),
                signature,
                device_id: get_device_id(),
                device_name: get_device_name(),
                device_type: DeviceType::Browser,
            };

            match api.complete_wallet_login(complete_request).await {
                Ok(response) => {
                    set_busy.set(false);
                    on_success.run(response.session_id.to_string());
                }
                Err(e) => {
                    set_error.set(Some(format!("Sign-in failed: {e}")));
                    set_busy.set(false);
                }
            }
        });
    });

    let on_passkey_submit = Callback::new(move |_: String| {
        let api = api.get_value();
        set_passkey_state.set(PasskeyState::Authenticating);
        set_error.set(None);

        leptos::task::spawn_local(async move {
            let challenge = match api.start_passkey_login().await {
                Ok(r) => r,
                Err(e) => {
                    set_error.set(Some(format!("Could not start sign-in: {e}")));
                    set_passkey_state.set(PasskeyState::Error(e.to_string()));
                    return;
                }
            };

            let credential = match get_credential(&challenge.options).await {
                Ok(cred) => cred,
                Err(e) => {
                    set_error.set(Some(format!("Authentication failed: {e}")));
                    set_passkey_state.set(PasskeyState::Error(e.to_string()));
                    return;
                }
            };

            let complete_request = CompletePasskeyLoginRequest {
                challenge_id: challenge.challenge_id,
                credential,
                device_id: get_device_id(),
                device_name: get_device_name(),
                device_type: DeviceType::Browser,
            };

            match api.complete_passkey_login(complete_request).await {
                Ok(response) => {
                    set_passkey_state.set(PasskeyState::Success);
                    on_success.run(response.session_id.to_string());
                }
                Err(e) => {
                    set_error.set(Some(format!("Sign-in failed: {e}")));
                    set_passkey_state.set(PasskeyState::Error(e.to_string()));
                }
            }
        });
    });

    view! {
        <div class="reauth-gate">
            <p class="form-help">
                "For your security, confirm it's you with a fresh passkey or wallet sign-in \
                 before this change takes effect."
            </p>
            <div class="ps-auth-tabs">
                <button
                    type="button"
                    class=move || if tab.get() == ReauthTab::Wallet {
                        "ps-auth-tab ps-auth-tab-active"
                    } else {
                        "ps-auth-tab"
                    }
                    on:click=move |_| set_tab.set(ReauthTab::Wallet)
                >
                    "Wallet"
                </button>
                <button
                    type="button"
                    class=move || if tab.get() == ReauthTab::Passkey {
                        "ps-auth-tab ps-auth-tab-active"
                    } else {
                        "ps-auth-tab"
                    }
                    on:click=move |_| set_tab.set(ReauthTab::Passkey)
                >
                    "Passkey"
                </button>
            </div>

            {move || error.get().map(|e| view! { <p style="color: var(--color-error)">{e}</p> })}

            <div>
                {move || match tab.get() {
                    ReauthTab::Wallet => view! {
                        <WalletConnectButton
                            on_connect=on_wallet_connect
                            button_text="Verify with Wallet".to_string()
                        />
                    }.into_any(),
                    ReauthTab::Passkey => view! {
                        <PasskeyAuthForm
                            is_registration=false
                            on_submit=on_passkey_submit
                            state=passkey_state
                        />
                    }.into_any(),
                }}
            </div>

            <div class="form-actions">
                <button
                    class="ps-btn ps-btn-secondary ps-btn-sm"
                    on:click=move |_| on_cancel.run(())
                    disabled=move || {
                        busy.get() || matches!(passkey_state.get(), PasskeyState::Authenticating)
                    }
                >
                    "Cancel"
                </button>
            </div>
        </div>
    }
}
