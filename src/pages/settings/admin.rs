//! Admin settings tab - server settings and user management (admin only).

use crate::api::{
    AdminUserInfo, ApiClient, Store, UpdateServerSettingsRequest, UpdateUserRoleRequest,
};
use leptos::prelude::*;
use types::ChainId;

use super::IconShield;

/// Admin tab - server settings and user management (admin only).
#[component]
pub fn AdminTab() -> impl IntoView {
    let api = use_context::<Signal<ApiClient>>().expect("ApiClient must be provided");

    // Safe mode state - true when the server booted with every plugin disabled.
    let (safe_mode, set_safe_mode) = signal(false);
    // Whether the safe-mode check itself failed - kept distinct from `safe_mode`
    // so a transient fetch error can't be mistaken for "plugins are fine".
    let (safe_mode_check_failed, set_safe_mode_check_failed) = signal(false);

    // Settings form state
    let (default_confirmations, set_default_confirmations) = signal("3".to_string());
    let (invoice_expiry, set_invoice_expiry) = signal("60".to_string());
    let (rate_limit, set_rate_limit) = signal("100".to_string());
    let (enabled_chain_ids, set_enabled_chain_ids) = signal(Vec::<ChainId>::new());
    // Whether the operator actually touched the chain list on this visit.
    //
    // The server treats a stored list as authoritative - every chain not in
    // it is refused - and answers `GET` with compiled-in *mainnet* defaults
    // when nothing is stored. Sending back whatever was loaded would
    // therefore write a list nobody chose, and on a testnet deployment that
    // list has no Sepolia in it, so the instance would stop accepting the
    // only chain it watches. Saving the billing store must not do that.
    let (chains_edited, set_chains_edited) = signal(false);
    let (settings_status, set_settings_status) = signal(String::new());

    // The store this instance bills its own subscriptions through. Empty
    // string means none - an instance that sells nothing to itself.
    let (billing_store, set_billing_store) = signal(String::new());
    // Whether the saved value is the one the server is actually running with.
    // It is read at boot, so a change sits pending until a restart, and an
    // admin looking at this page needs to be able to tell the difference.
    let (billing_store_active, set_billing_store_active) = signal(true);
    let (stores, set_stores) = signal(Vec::<Store>::new());

    // User list state
    let (users, set_users) = signal(Vec::<AdminUserInfo>::new());
    let (user_total, set_user_total) = signal(0i64);
    let (user_status, set_user_status) = signal(String::new());

    // All available networks
    // EVM chains this server can be told to enable. The name is carried
    // alongside because a CAIP-2 identifier does not imply one.
    let all_networks: Vec<(ChainId, &'static str)> = vec![
        (ChainId::evm(1), "Ethereum"),
        (ChainId::evm(10), "Optimism"),
        (ChainId::evm(137), "Polygon"),
        (ChainId::evm(42161), "Arbitrum"),
        (ChainId::evm(8453), "Base"),
        (ChainId::evm(56), "BSC"),
        (ChainId::evm(43114), "Avalanche"),
        (ChainId::evm(250), "Fantom"),
        (ChainId::evm(100), "Gnosis"),
        (ChainId::evm(324), "zkSync Era"),
        (ChainId::evm(59144), "Linea"),
        (ChainId::evm(534352), "Scroll"),
    ];

    // Load settings and users on mount
    leptos::task::spawn_local({
        let api = api.get_untracked();
        async move {
            if let Ok(settings) = api.get_server_settings().await {
                set_default_confirmations.set(settings.default_confirmations.to_string());
                set_invoice_expiry.set(settings.invoice_expiry_minutes.to_string());
                set_rate_limit.set(settings.rate_limit_rpm.to_string());
                set_enabled_chain_ids.set(settings.enabled_chain_ids);
                set_billing_store.set(
                    settings
                        .billing_store_id
                        .map(|id| id.0.to_string())
                        .unwrap_or_default(),
                );
                set_billing_store_active.set(settings.billing_store_id_active);
            }
            // Offered as a list rather than a UUID field. An operator should
            // not have to copy an identifier out of a URL to configure where
            // their own revenue lands, and a typo there is silent until a
            // merchant cannot pay.
            if let Ok(list) = api.list_stores().await {
                set_stores.set(list.into_iter().filter(|s| !s.archived).collect());
            }
            if let Ok(resp) = api.list_users(0, 100).await {
                set_user_total.set(resp.total);
                set_users.set(resp.users);
            }
            match api.get_safe_mode().await {
                Ok(status) => set_safe_mode.set(status.safe_mode),
                Err(_) => set_safe_mode_check_failed.set(true),
            }
        }
    });

    // Save settings handler
    let save_settings = move |_| {
        let api = api.get_untracked();
        let confirmations = default_confirmations
            .get_untracked()
            .parse::<i32>()
            .unwrap_or(3);
        let expiry = invoice_expiry.get_untracked().parse::<i32>().unwrap_or(60);
        let rpm = rate_limit.get_untracked().parse::<i32>().unwrap_or(100);
        // `None` means "not talking about chains", which is not the same as
        // an empty list. Only a deliberate edit sends one.
        let chains = chains_edited
            .get_untracked()
            .then(|| enabled_chain_ids.get_untracked());
        let billing = billing_store.get_untracked();
        leptos::task::spawn_local(async move {
            // Always `Some(..)`: this form knows about the field, so it is
            // always talking about it. The inner option is the value - `None`
            // clears the setting. An absent field would mean "leave it
            // alone", which is what an older client sends and is not what a
            // save from this page means.
            let billing_store_id = Some(if billing.is_empty() {
                None
            } else {
                billing.parse().ok().map(types::StoreId)
            });

            let request = UpdateServerSettingsRequest {
                default_confirmations: confirmations,
                invoice_expiry_minutes: expiry,
                rate_limit_rpm: rpm,
                enabled_chain_ids: chains,
                billing_store_id,
            };
            match api.update_server_settings(&request).await {
                Ok(()) => {
                    set_chains_edited.set(false);
                    // Saved is not applied. Saying only "saved" would leave an
                    // admin believing the server is billing on the store they
                    // just picked, which it is not until it restarts.
                    set_settings_status.set(
                        "Settings saved. The billing store takes effect when the server restarts."
                            .to_string(),
                    );
                    set_billing_store_active.set(false);
                }
                Err(e) => set_settings_status.set(format!("Error: {}", e)),
            }
        });
    };

    // Toggle network handler
    let toggle_network = move |chain_id: ChainId| {
        set_chains_edited.set(true);
        set_enabled_chain_ids.update(|ids| {
            if ids.contains(&chain_id) {
                ids.retain(|id| id != &chain_id);
            } else {
                ids.push(chain_id);
            }
        });
    };

    // Role change handler
    let change_role = move |user_id: String, new_role: String| {
        let api = api.get_untracked();
        leptos::task::spawn_local(async move {
            let request = UpdateUserRoleRequest { role: new_role };
            match api.update_user_role(&user_id, &request).await {
                Ok(()) => {
                    set_user_status.set("Role updated".to_string());
                    if let Ok(resp) = api.list_users(0, 100).await {
                        set_users.set(resp.users);
                    }
                }
                Err(e) => set_user_status.set(format!("Error: {}", e)),
            }
        });
    };

    // Lock/unlock handler
    let toggle_lock = move |user_id: String, is_locked: bool| {
        let api = api.get_untracked();
        leptos::task::spawn_local(async move {
            let result = if is_locked {
                api.unlock_user(&user_id).await
            } else {
                api.lock_user(&user_id).await
            };
            match result {
                Ok(()) => {
                    if let Ok(resp) = api.list_users(0, 100).await {
                        set_users.set(resp.users);
                    }
                }
                Err(e) => set_user_status.set(format!("Error: {}", e)),
            }
        });
    };

    view! {
        <div class="settings-tab-admin">
            {move || {
                if safe_mode.get() {
                    view! {
                        <div class="alert alert-error">
                            <strong>"⚠ SAFE MODE: every plugin is disabled"</strong>
                            <p>
                                "The server booted with ETHPAY_DISABLE_PLUGINS set (or the "
                                "--disable-plugins flag). No plugin is running - including "
                                "billing, if it is installed as one. Plugins are not "
                                "uninstalled and their data is untouched: clear the flag and "
                                "restart to bring them back."
                            </p>
                        </div>
                    }.into_any()
                } else if safe_mode_check_failed.get() {
                    view! {
                        <div class="alert alert-warning">
                            <strong>"⚠ Could not confirm plugin status"</strong>
                            <p>
                                "The safe-mode check itself failed, so whether every plugin is "
                                "disabled for this boot is unknown - this is not the same as "
                                "confirming plugins are running normally."
                            </p>
                        </div>
                    }.into_any()
                } else {
                    view! { <span></span> }.into_any()
                }
            }}

            <div class="admin-warning">
                <IconShield />
                <div>
                    <strong>"Server Administration"</strong>
                    <p>"These settings affect the entire server. Changes may require a restart to take effect."</p>
                </div>
            </div>

            // User Management
            <div class="ps-card">
                <div class="ps-card-header">
                    <h3>"User Management"</h3>
                    <span class="badge">{move || format!("{} users", user_total.get())}</span>
                </div>
                <div class="ps-card-body">
                    {move || {
                        let status = user_status.get();
                        if status.is_empty() {
                            view! { <span></span> }.into_any()
                        } else {
                            view! { <p class="form-help">{status}</p> }.into_any()
                        }
                    }}
                    <div class="admin-users-table">
                        <table class="data-table">
                            <thead>
                                <tr>
                                    <th>"User"</th>
                                    <th>"Role"</th>
                                    <th>"Created"</th>
                                    <th>"Actions"</th>
                                </tr>
                            </thead>
                            <tbody>
                                {move || users.get().into_iter().map(|user| {
                                    let user_id_role = user.id.clone();
                                    let user_id_lock = user.id.clone();
                                    let is_locked = user.locked_until.is_some();
                                    let display_name = user.email.clone()
                                        .or(user.primary_wallet_address.clone().map(|w| {
                                            let prefix = w.get(..6).unwrap_or(w.as_str());
                                            let suffix = w.get(w.len().saturating_sub(4)..).unwrap_or("");
                                            format!("{prefix}...{suffix}")
                                        }))
                                        .unwrap_or_else(|| user.id.get(..8).unwrap_or(user.id.as_str()).to_string());
                                    let current_role = user.role.clone();
                                    let created_date = user.created_at.format("%Y-%m-%d").to_string();
                                    view! {
                                        <tr class=if is_locked { "user-row locked" } else { "user-row" }>
                                            <td class="user-cell">
                                                <span class="user-name">{display_name}</span>
                                            </td>
                                            <td>
                                                <select
                                                    class="form-input form-input-sm"
                                                    on:change=move |ev| {
                                                        let new_role = event_target_value(&ev);
                                                        change_role(user_id_role.clone(), new_role);
                                                    }
                                                >
                                                    <option value="server_admin" selected=current_role == "server_admin">"Server Admin"</option>
                                                    <option value="user" selected=current_role == "user">"User"</option>
                                                </select>
                                            </td>
                                            <td class="date-cell">{created_date}</td>
                                            <td>
                                                <button
                                                    class=if is_locked { "ps-btn ps-btn-xs ps-btn-success" } else { "ps-btn ps-btn-xs ps-btn-warning" }
                                                    on:click=move |_| toggle_lock(user_id_lock.clone(), is_locked)
                                                >
                                                    {if is_locked { "Unlock" } else { "Lock" }}
                                                </button>
                                            </td>
                                        </tr>
                                    }
                                }).collect_view()}
                            </tbody>
                        </table>
                    </div>
                </div>
            </div>

            // Billing
            //
            // Its own card rather than a field among the payment defaults:
            // this is the only setting on the page that decides where money
            // is invoiced, and it is the only one that does not take effect
            // until a restart.
            <div class="ps-card">
                <div class="ps-card-header">
                    <h3>"Billing"</h3>
                </div>
                <div class="ps-card-body">
                    <div class="form-group">
                        <label class="form-label">"Subscription store"</label>
                        <select
                            class="form-input"
                            prop:value=move || billing_store.get()
                            on:change=move |ev| set_billing_store.set(event_target_value(&ev))
                        >
                            <option value="">"None - this server bills nothing for itself"</option>
                            <For
                                each=move || stores.get()
                                key=|store| store.id
                                let:store
                            >
                                <option value=store.id.to_string()>{store.name.clone()}</option>
                            </For>
                        </select>
                        <p class="form-help">
                            "Subscription invoices are issued on this store, and payments to it \
                             are what a billing plugin is told about. It needs an enabled payment \
                             method whose wallet resolves, or it will be refused."
                        </p>
                        <Show when=move || !billing_store_active.get()>
                            <p class="form-help" style="color: var(--color-warning);">
                                "Pending restart - the server is not billing on this store yet."
                            </p>
                        </Show>
                    </div>
                </div>
            </div>

            // Payment Defaults
            <div class="ps-card">
                <div class="ps-card-header">
                    <h3>"Payment Defaults"</h3>
                </div>
                <div class="ps-card-body">
                    <div class="settings-grid">
                        <div class="form-group">
                            <label class="form-label">"Required Confirmations"</label>
                            <input
                                type="number"
                                class="form-input"
                                min="1"
                                max="100"
                                prop:value=move || default_confirmations.get()
                                on:input=move |ev| set_default_confirmations.set(event_target_value(&ev))
                            />
                            <p class="form-help">"Block confirmations before payment is final"</p>
                        </div>

                        <div class="form-group">
                            <label class="form-label">"Invoice Expiry (minutes)"</label>
                            <input
                                type="number"
                                class="form-input"
                                min="5"
                                max="1440"
                                prop:value=move || invoice_expiry.get()
                                on:input=move |ev| set_invoice_expiry.set(event_target_value(&ev))
                            />
                            <p class="form-help">"Default expiration time for new invoices"</p>
                        </div>

                        <div class="form-group">
                            <label class="form-label">"Rate Limit (req/min)"</label>
                            <input
                                type="number"
                                class="form-input"
                                min="10"
                                max="1000"
                                prop:value=move || rate_limit.get()
                                on:input=move |ev| set_rate_limit.set(event_target_value(&ev))
                            />
                            <p class="form-help">"API rate limit per IP address"</p>
                        </div>
                    </div>
                </div>
            </div>

            // Enabled Networks
            <div class="ps-card">
                <div class="ps-card-header">
                    <h3>"Enabled Networks"</h3>
                </div>
                <div class="ps-card-body">
                    <p class="form-help" style="margin-bottom: 16px;">
                        "Networks available for payment processing. Disabled networks cannot be used by any store."
                    </p>
                    <div class="admin-networks-grid">
                        {all_networks.into_iter().map(|(chain_id, name)| {
                            let for_class = chain_id.clone();
                            let for_checked = chain_id.clone();
                            let for_toggle = chain_id.clone();
                            view! {
                                <div class=move || if enabled_chain_ids.get().contains(&for_class) { "admin-network-item enabled" } else { "admin-network-item" }>
                                    <div class="admin-network-info">
                                        <span class="admin-network-name">{name}</span>
                                        <span class="admin-network-chain-id">"Chain: "{chain_id.to_string()}</span>
                                    </div>
                                    <label class="toggle">
                                        <input
                                            type="checkbox"
                                            prop:checked=move || enabled_chain_ids.get().contains(&for_checked)
                                            on:change=move |_| toggle_network(for_toggle.clone())
                                        />
                                        <span class="toggle-slider"></span>
                                    </label>
                                </div>
                            }
                        }).collect_view()}
                    </div>
                </div>
            </div>

            // Maintenance
            <div class="ps-card">
                <div class="ps-card-header">
                    <h3>"Maintenance"</h3>
                </div>
                <div class="ps-card-body">
                    <div class="admin-actions">
                        <div class="admin-action">
                            <div class="admin-action-info">
                                <span class="admin-action-title">"Clear cache"</span>
                                <span class="admin-action-desc">"Clear all cached data including exchange rates"</span>
                            </div>
                            // None of the three maintenance actions call the server. Left
                            // enabled, an admin has no way to tell a no-op from a restart
                            // that happened.
                            <button
                                class="ps-btn ps-btn-secondary ps-btn-sm"
                                disabled=true
                                title="Clear cache is not implemented yet"
                            >
                                "Clear cache"
                            </button>
                        </div>

                        <div class="admin-action">
                            <div class="admin-action-info">
                                <span class="admin-action-title">"Restart monitor"</span>
                                <span class="admin-action-desc">"Restart the EVM chain monitor service"</span>
                            </div>
                            <button
                                class="ps-btn ps-btn-secondary ps-btn-sm"
                                disabled=true
                                title="Restarting the monitor is not implemented yet"
                            >
                                "Restart"
                            </button>
                        </div>

                        <div class="admin-action">
                            <div class="admin-action-info">
                                <span class="admin-action-title">"Export data"</span>
                                <span class="admin-action-desc">"Export all server data as JSON backup"</span>
                            </div>
                            <button
                                class="ps-btn ps-btn-secondary ps-btn-sm"
                                disabled=true
                                title="Export is not implemented yet"
                            >
                                "Export"
                            </button>
                        </div>
                    </div>
                </div>
            </div>

            // Save + status
            <div class="form-actions">
                {move || {
                    let status = settings_status.get();
                    if status.is_empty() {
                        view! { <span></span> }.into_any()
                    } else {
                        view! { <span class="form-status">{status}</span> }.into_any()
                    }
                }}
                <button class="ps-btn ps-btn-primary" on:click=save_settings>"Save server settings"</button>
            </div>
        </div>
    }
}
