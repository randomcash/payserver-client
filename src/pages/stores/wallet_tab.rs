//! Wallet rotation tab: rotate the xpub a store's payment methods derive
//! from.
//!
//! This is the one wallet operation that previously had no client binding at
//! all - promoting a wallet to primary and pinning a payment method to a
//! different key both already had one. It is also the one the runbook says
//! matters most: key compromise, a stolen device, a personnel change,
//! scheduled rotation. Those are exactly the moments someone is least likely
//! to be in a position to compose the `curl` call by hand.

use leptos::prelude::*;

use crate::api::{ApiClient, ApiError, RotateWalletRequest, RotateWalletResponse};
use crate::util::chain_name;

use super::format_date;

/// Detects a pasted extended *private* key.
///
/// The server refuses this the same way it refuses any other malformed
/// input - a bare 400, on the BIP-32 version byte, with no body - so without
/// this check the UI would show the same blank "HTTP error 400: " for a
/// pasted `xprv` as for a typo. This names the specific, more likely mistake.
fn looks_like_a_private_key(xpub: &str) -> bool {
    let trimmed = xpub.trim();
    trimmed.starts_with("xprv") || trimmed.starts_with("tprv")
}

/// Names of stores (other than `excluding`) whose resolved wallet matches
/// `old_xpub_masked`.
///
/// Compared by masked xpub, not wallet id: the masked value is the only
/// identifier `GET /stores/{id}/wallet` exposes for a store this caller did
/// not just rotate, and it keeps enough of a base58 key (8 leading + 8
/// trailing characters) that two different wallets sharing one by chance is
/// not a real concern here.
fn stores_sharing_wallet<'a>(
    old_xpub_masked: &str,
    excluding: &str,
    resolved: impl Iterator<Item = (&'a str, &'a str, &'a str)>,
) -> Vec<String> {
    resolved
        .filter(|(store_id, _, masked)| *store_id != excluding && *masked == old_xpub_masked)
        .map(|(_, name, _)| name.to_string())
        .collect()
}

/// Wallet rotation tab.
#[component]
pub fn WalletTab(store_id: String) -> impl IntoView {
    let api = use_context::<Signal<ApiClient>>().expect("ApiClient must be provided");

    let (refresh, set_refresh) = signal(0u32);
    let sid_fetch = store_id.clone();
    let wallet_resource = LocalResource::new(move || {
        refresh.track();
        let api = api.get();
        let id = sid_fetch.clone();
        async move { api.get_store_wallet(&id).await }
    });

    let (new_xpub, set_new_xpub) = signal(String::new());
    let (reason, set_reason) = signal(String::new());
    let (confirming, set_confirming) = signal(false);
    let (loading_warnings, set_loading_warnings) = signal(false);
    let (outstanding, set_outstanding) = signal(0i64);
    let (other_stores, set_other_stores) = signal(Vec::<String>::new());
    let (rotating, set_rotating) = signal(false);
    let (rotate_error, set_rotate_error) = signal(None::<String>);
    let (last_rotation, set_last_rotation) = signal(None::<RotateWalletResponse>);

    let sid_confirm = store_id.clone();
    let on_start_confirm = move |_| {
        let xpub = new_xpub.get_untracked().trim().to_string();
        if xpub.is_empty() {
            return;
        }
        set_rotate_error.set(None);
        set_last_rotation.set(None);
        set_confirming.set(true);
        set_loading_warnings.set(true);
        let api = api.get();
        let sid = sid_confirm.clone();
        leptos::task::spawn_local(async move {
            // In-flight invoices (pending, processing) keep collecting on the
            // old key after rotation - that is correct behaviour, and it is
            // the one most likely to surprise someone rotating because a key
            // just leaked. `limit=1` because only `total` is needed here.
            let mut outstanding_count = 0i64;
            for status in ["pending", "processing"] {
                if let Ok(page) = api
                    .list_invoices(Some(&sid), Some(status), None, None, Some(1), Some(0))
                    .await
                {
                    outstanding_count += page.total;
                }
            }
            let _ = set_outstanding.try_set(outstanding_count);

            // Rotation is per store, not per account: a wallet can back
            // several stores, and only this one is about to move. Find the
            // others still pointing at the key this store is leaving.
            let mut sharing = Vec::new();
            if let Ok(current) = api.get_store_wallet(&sid).await {
                let old_masked = current.wallet.xpub_masked;
                if let Ok(stores) = api.list_stores().await {
                    let mut resolved = Vec::with_capacity(stores.len());
                    for store in &stores {
                        let other_id = store.id.to_string();
                        if other_id == sid {
                            continue;
                        }
                        if let Ok(other) = api.get_store_wallet(&other_id).await {
                            resolved.push((other_id, store.name.clone(), other.wallet.xpub_masked));
                        }
                    }
                    sharing = stores_sharing_wallet(
                        &old_masked,
                        &sid,
                        resolved
                            .iter()
                            .map(|(id, name, masked)| (id.as_str(), name.as_str(), masked.as_str())),
                    );
                }
            }
            let _ = set_other_stores.try_set(sharing);
            let _ = set_loading_warnings.try_set(false);
        });
    };

    let sid_rotate = store_id.clone();
    let on_confirm_rotate = move |_| {
        let xpub = new_xpub.get_untracked().trim().to_string();
        let reason_value = reason.get_untracked().trim().to_string();
        let api = api.get();
        let sid = sid_rotate.clone();
        set_rotating.set(true);
        set_rotate_error.set(None);
        leptos::task::spawn_local(async move {
            let req = RotateWalletRequest {
                xpub: xpub.clone(),
                reason: (!reason_value.is_empty()).then_some(reason_value),
                namespace: "eip155".to_string(),
            };
            match api.rotate_store_wallet(&sid, &req).await {
                Ok(response) => {
                    let _ = set_last_rotation.try_set(Some(response));
                    let _ = set_new_xpub.try_set(String::new());
                    let _ = set_reason.try_set(String::new());
                    let _ = set_confirming.try_set(false);
                    let _ = set_refresh.try_update(|c| *c += 1);
                }
                Err(e) => {
                    let message = match &e {
                        ApiError::Http { status: 400, .. } if looks_like_a_private_key(&xpub) => {
                            "That is a private key (xprv), not a public one. Rotation needs an \
                             extended PUBLIC key - the private key should never leave the wallet \
                             software that holds it."
                                .to_string()
                        }
                        ApiError::Http { status: 400, .. } => {
                            "That doesn't look like a valid extended public key. Check for a \
                             typo or a truncated paste - a full xpub is over 100 characters."
                                .to_string()
                        }
                        other => other.to_string(),
                    };
                    let _ = set_rotate_error.try_set(Some(message));
                }
            }
            let _ = set_rotating.try_set(false);
        });
    };

    view! {
        <div class="store-tab-wallet">
            <Suspense fallback=move || view! { <div class="loading-state">"Loading wallet..."</div> }>
                {move || wallet_resource.get().map(|result| match &*result {
                    Ok(wallet) => {
                        let origin = if wallet.is_override { "Store override" } else { "Account primary" };
                        view! {
                            <div class="ps-card">
                                <div class="ps-card-header"><h3>"Current wallet"</h3></div>
                                <div class="ps-card-body">
                                    <div class="form-group">
                                        <label class="form-label">"Extended Public Key"</label>
                                        <div class="form-static">
                                            <code class="wallet-address-full">{wallet.wallet.xpub_masked.clone()}</code>
                                        </div>
                                    </div>
                                    <div class="form-group">
                                        <label class="form-label">"Derivation Index"</label>
                                        <div class="form-static"><code>{wallet.wallet.derivation_index}</code></div>
                                    </div>
                                    <div class="form-group">
                                        <label class="form-label">"Source"</label>
                                        <div class="form-static">{origin}</div>
                                    </div>
                                </div>
                            </div>
                        }.into_any()
                    }
                    Err(_) => view! {
                        <div class="ps-card">
                            <div class="ps-card-body">
                                <p class="form-help">
                                    "No wallet currently resolves for this store on eip155 - no \
                                     override, and no account primary."
                                </p>
                            </div>
                        </div>
                    }.into_any(),
                })}
            </Suspense>

            <div class="ps-card">
                <div class="ps-card-header">
                    <h3>"Rotate wallet key"</h3>
                </div>
                <div class="ps-card-body">
                    <div class="form-group">
                        <label class="form-label">"New extended public key (xpub)"</label>
                        <input
                            type="text"
                            class="form-input"
                            placeholder="xpub..."
                            prop:value=move || new_xpub.get()
                            on:input=move |ev| set_new_xpub.set(event_target_value(&ev))
                            disabled=move || confirming.get()
                        />
                        <p class="form-help">
                            "An extended PUBLIC key. The server refuses a private key (xprv) on \
                             its version byte - it never holds one."
                        </p>
                    </div>
                    <div class="form-group">
                        <label class="form-label">"Reason "<span class="text-muted">"(optional)"</span></label>
                        <input
                            type="text"
                            class="form-input"
                            placeholder="key compromise, scheduled rotation, ..."
                            prop:value=move || reason.get()
                            on:input=move |ev| set_reason.set(event_target_value(&ev))
                            disabled=move || confirming.get()
                        />
                    </div>

                    {move || rotate_error.get().map(|msg| view! {
                        <p style="color: var(--color-error)">{msg}</p>
                    })}

                    {move || (!confirming.get()).then(|| {
                        let on_start_confirm = on_start_confirm.clone();
                        view! {
                        <div class="form-actions">
                            <button
                                class="ps-btn ps-btn-primary ps-btn-sm"
                                on:click=on_start_confirm
                                disabled=move || new_xpub.get().trim().is_empty()
                            >
                                "Rotate"
                            </button>
                        </div>
                    }})}

                    {move || confirming.get().then(|| {
                        let on_confirm_rotate = on_confirm_rotate.clone();
                        view! {
                        <div class="rotation-confirm">
                            <div class="addresses-info">
                                <IconInfo />
                                <p>
                                    "In-flight invoices keep collecting on the old key. Pending \
                                     and processing invoices continue to detect payments on \
                                     addresses derived from the current key after this rotation - \
                                     that is correct and expected, not a bug."
                                    {move || if loading_warnings.get() {
                                        " Checking how many...".to_string()
                                    } else {
                                        format!(" Currently outstanding: {}.", outstanding.get())
                                    }}
                                </p>
                            </div>

                            {move || (!loading_warnings.get() && !other_stores.get().is_empty()).then(|| view! {
                                <div class="addresses-info">
                                    <IconInfo />
                                    <p>
                                        "Rotation is per store, not per account. A wallet can back \
                                         several stores, so this moves only this store's payment \
                                         methods. Still on the current key: "
                                        {other_stores.get().join(", ")}
                                        "."
                                    </p>
                                </div>
                            })}

                            <div class="addresses-info">
                                <IconInfo />
                                <p>
                                    "Derivation indices are not reset. If the account has used \
                                     this xpub before, rotation resumes where it left off; a new \
                                     key starts at index 0."
                                </p>
                            </div>

                            <div class="form-actions">
                                <button
                                    class="ps-btn ps-btn-primary ps-btn-sm"
                                    on:click=on_confirm_rotate
                                    disabled=move || rotating.get() || loading_warnings.get()
                                >
                                    {move || if rotating.get() { "Rotating..." } else { "Confirm rotation" }}
                                </button>
                                <button
                                    class="ps-btn ps-btn-secondary ps-btn-sm"
                                    on:click=move |_| set_confirming.set(false)
                                    disabled=move || rotating.get()
                                >
                                    "Cancel"
                                </button>
                            </div>
                        </div>
                    }})}
                </div>
            </div>

            {move || last_rotation.get().map(|response| {
                let rotations = response.rotations.clone();
                view! {
                    <div class="ps-card">
                        <div class="ps-card-header">
                            <h3>"Rotated "{response.methods_rotated}" payment method(s)"</h3>
                        </div>
                        <div class="ps-card-body">
                            <p class="form-help">"New key: "<code>{response.new_xpub_masked.clone()}</code></p>
                            <div class="payment-methods-table-container">
                                <table class="payment-methods-table">
                                    <thead>
                                        <tr>
                                            <th>"Asset"</th>
                                            <th>"Network"</th>
                                            <th>"Previous key"</th>
                                            <th>"Previous index"</th>
                                            <th>"Rotated at"</th>
                                        </tr>
                                    </thead>
                                    <tbody>
                                        {rotations.into_iter().map(|rotation| {
                                            let network = rotation.chain_id
                                                .as_ref()
                                                .map(|id| chain_name(id).to_string())
                                                .unwrap_or_else(|| "-".to_string());
                                            let asset = rotation.asset_symbol.clone().unwrap_or_else(|| "-".to_string());
                                            let rotated_display = format_date(&rotation.rotated_at.to_rfc3339());
                                            view! {
                                                <tr>
                                                    <td>{asset}</td>
                                                    <td>{network}</td>
                                                    <td><code>{rotation.previous_xpub_masked}</code></td>
                                                    <td><code>{rotation.previous_derivation_index}</code></td>
                                                    <td>{rotated_display}</td>
                                                </tr>
                                            }
                                        }).collect_view()}
                                    </tbody>
                                </table>
                            </div>
                        </div>
                    </div>
                }
            })}
        </div>
    }
}

#[component]
fn IconInfo() -> impl IntoView {
    view! {
        <svg xmlns="http://www.w3.org/2000/svg" width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
            <circle cx="12" cy="12" r="10"></circle>
            <line x1="12" y1="16" x2="12" y2="12"></line>
            <line x1="12" y1="8" x2="12.01" y2="8"></line>
        </svg>
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // =========================================================================
    // looks_like_a_private_key
    // =========================================================================

    #[test]
    fn detects_mainnet_and_testnet_private_key_prefixes() {
        assert!(looks_like_a_private_key("xprv9s21ZrQH143K3QTDL4LXw2F7HEK3wJUD2nW2nRk4stbPy6cq4o..."));
        assert!(looks_like_a_private_key("tprv8ZgxMBicQKsPd..."));
    }

    #[test]
    fn accepts_leading_and_trailing_whitespace() {
        assert!(looks_like_a_private_key("  xprv9s21ZrQH143K3QTDL4LXw2F7HEK3wJUD2nW2nRk4stbPy6cq4o...  "));
    }

    #[test]
    fn does_not_flag_a_public_key() {
        assert!(!looks_like_a_private_key(
            "xpub6DCoCpSuQZB2jawqnGMEPS63ePKWkwWPH4TU45Q7LPXWuNd8TMtVxRrgjtEshuqpK3mdhaWHPFsBngh5GFZaM6si3yZdUsT8ddYM3PwnATt"
        ));
    }

    #[test]
    fn does_not_flag_unrelated_garbage() {
        assert!(!looks_like_a_private_key("not-a-key-at-all"));
        assert!(!looks_like_a_private_key(""));
    }

    // =========================================================================
    // stores_sharing_wallet
    // =========================================================================

    #[test]
    fn finds_other_stores_on_the_same_masked_xpub() {
        let resolved = vec![
            ("store-2", "Coffee Shop", "xpub6D4B...eacc"),
            ("store-3", "Book Store", "xpub6D4B...eacc"),
            ("store-4", "Flower Shop", "xpub6ZZZ...9999"),
        ];
        let sharing = stores_sharing_wallet(
            "xpub6D4B...eacc",
            "store-1",
            resolved.into_iter(),
        );
        assert_eq!(sharing, vec!["Coffee Shop".to_string(), "Book Store".to_string()]);
    }

    #[test]
    fn excludes_the_store_being_rotated_even_if_it_appears_in_the_input() {
        let resolved = vec![("store-1", "This Store", "xpub6D4B...eacc")];
        let sharing = stores_sharing_wallet("xpub6D4B...eacc", "store-1", resolved.into_iter());
        assert!(sharing.is_empty());
    }

    #[test]
    fn empty_when_nothing_else_shares_the_key() {
        let resolved = vec![("store-2", "Coffee Shop", "xpub6ZZZ...9999")];
        let sharing = stores_sharing_wallet("xpub6D4B...eacc", "store-1", resolved.into_iter());
        assert!(sharing.is_empty());
    }
}
