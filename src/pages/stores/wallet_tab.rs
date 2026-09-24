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

/// Turns a failed rotation into the message shown to the operator.
///
/// The server's own message wins whenever it sent one - a real diagnosis
/// beats a guess, even when the input also looks like an xprv, since a
/// specific version-byte message from the server is more informative than
/// the canned text below. The private-key guess exists only for the common
/// case where the server's 400 body is empty, since a blank "HTTP error 400:
/// " does not tell anyone pasting an xprv what they got wrong.
fn rotation_error_message(err: &ApiError, xpub: &str) -> String {
    match err {
        ApiError::Http {
            status: 400,
            message,
        } if !message.trim().is_empty() => message.clone(),
        ApiError::Http { status: 400, .. } if looks_like_a_private_key(xpub) => {
            "That is a private key (xprv), not a public one. Rotation needs an \
             extended PUBLIC key - the private key should never leave the wallet \
             software that holds it."
                .to_string()
        }
        ApiError::Http { status: 400, .. } => {
            "That doesn't look like a valid extended public key, or every \
             payment method already uses it. Check for a typo or a truncated \
             paste - a full xpub is over 100 characters."
                .to_string()
        }
        other => other.to_string(),
    }
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

/// The outstanding pending/processing invoice count for `store_id`, or `None`
/// if either count could not be fetched.
///
/// `None` is a distinct state from `Some(0)`: this backs the warning that a
/// rotation does not stop old-address collection, and a fetch that failed
/// must never be shown as "0 outstanding" - that reads as safe to someone
/// rotating because a key just leaked, when it actually means the check
/// never ran.
async fn fetch_outstanding_invoice_count(api: &ApiClient, store_id: &str) -> Option<i64> {
    let mut total = 0i64;
    for status in ["pending", "processing"] {
        let page = api
            .list_invoices(Some(store_id), Some(status), None, None, Some(1), Some(0))
            .await
            .ok()?;
        total += page.total;
    }
    Some(total)
}

/// Other stores still resolving to `store_id`'s current wallet, or `None` if
/// the check could not complete.
///
/// Any failed lookup along the way - the store's own wallet, the store list,
/// or any one candidate store's wallet - aborts with `None` rather than
/// returning a list missing that entry: a rotation made in response to a
/// compromise treats "no other stores" and "couldn't check" as the same
/// green light otherwise, and a silently short list is worse than an
/// explicit "could not verify."
async fn fetch_stores_sharing_wallet(api: &ApiClient, store_id: &str) -> Option<Vec<String>> {
    let current = api.get_store_wallet(store_id).await.ok()?;
    let stores = api.list_stores().await.ok()?;
    let mut resolved = Vec::with_capacity(stores.len());
    for store in &stores {
        let other_id = store.id.to_string();
        if other_id == store_id {
            continue;
        }
        let other = api.get_store_wallet(&other_id).await.ok()?;
        resolved.push((other_id, store.name.clone(), other.wallet.xpub_masked));
    }
    Some(stores_sharing_wallet(
        &current.wallet.xpub_masked,
        store_id,
        resolved
            .iter()
            .map(|(id, name, masked)| (id.as_str(), name.as_str(), masked.as_str())),
    ))
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
    let (outstanding, set_outstanding) = signal(None::<i64>);
    let (other_stores, set_other_stores) = signal(None::<Vec<String>>);
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
            // just leaked.
            let outstanding_count = fetch_outstanding_invoice_count(&api, &sid).await;
            let _ = set_outstanding.try_set(outstanding_count);

            // Rotation is per store, not per account: a wallet can back
            // several stores, and only this one is about to move. Find the
            // others still pointing at the key this store is leaving.
            let sharing = fetch_stores_sharing_wallet(&api, &sid).await;
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
                // Not a UI gap: a store cannot hold a payment method on any
                // chain family other than eip155 today - the server refuses
                // to create one on a chain with no adapter registered, for
                // every chain family besides EVM, so eip155 is always the
                // whole store. A selector here would offer a namespace that
                // can never have anything to rotate.
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
                    let message = rotation_error_message(&e, &xpub);
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
                    Err(ApiError::Http { status: 404, .. }) => view! {
                        <div class="ps-card">
                            <div class="ps-card-body">
                                <p class="form-help">
                                    "No wallet currently resolves for this store on eip155 - no \
                                     override, and no account primary."
                                </p>
                            </div>
                        </div>
                    }.into_any(),
                    Err(_) => view! {
                        <div class="ps-card">
                            <div class="ps-card-body">
                                <p class="form-help" style="color: var(--color-error)">
                                    "Could not load the current wallet. This is a fetch failure, \
                                     not confirmation that no wallet is configured - reload before \
                                     relying on this page."
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
                                        match outstanding.get() {
                                            Some(n) => format!(" Currently outstanding: {}.", n),
                                            None => " Could not check how many - the fetch \
                                                      failed. Check the invoices list for this \
                                                      store manually before proceeding."
                                                .to_string(),
                                        }
                                    }}
                                </p>
                            </div>

                            {move || (!loading_warnings.get()).then(|| match other_stores.get() {
                                Some(names) if !names.is_empty() => view! {
                                    <div class="addresses-info">
                                        <IconInfo />
                                        <p>
                                            "Rotation is per store, not per account. A wallet can \
                                             back several stores, so this moves only this store's \
                                             payment methods. Still on the current key: "
                                            {names.join(", ")}
                                            "."
                                        </p>
                                    </div>
                                }.into_any(),
                                Some(_) => ().into_any(),
                                None => view! {
                                    <div class="addresses-info">
                                        <IconInfo />
                                        <p style="color: var(--color-error)">
                                            "Could not check whether other stores share this \
                                             wallet - the fetch failed. Check manually before \
                                             treating a compromise response as complete."
                                        </p>
                                    </div>
                                }.into_any(),
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
    use crate::api::client::TestTransport;
    use std::sync::{Arc, Mutex};

    /// Polls `fut` to completion against a `TestTransport` that answers
    /// synchronously, mirroring `api::client::stores`'s own `block_on` - see
    /// that module for why a full executor would be dead weight here.
    fn block_on<F: std::future::Future>(fut: F) -> F::Output {
        let mut fut = std::pin::pin!(fut);
        let waker = std::task::Waker::noop();
        let mut cx = std::task::Context::from_waker(waker);
        loop {
            if let std::task::Poll::Ready(value) = fut.as_mut().poll(&mut cx) {
                return value;
            }
        }
    }

    fn sample_wallet_json(store_id: &str, xpub_masked: &str) -> serde_json::Value {
        serde_json::json!({
            "store_id": store_id,
            "id": "11111111-1111-1111-1111-111111111111",
            "user_id": "00000000-0000-0000-0000-000000000001",
            "namespace": "eip155",
            "xpub_masked": xpub_masked,
            "derivation_index": 0,
            "name": null,
            "is_primary": false,
            "is_override": true,
            "created_at": "2026-01-01T00:00:00Z",
        })
    }

    fn sample_store_json(id: &str, name: &str) -> serde_json::Value {
        serde_json::json!({
            "id": id,
            "name": name,
            "website": null,
            "owner_id": "00000000-0000-0000-0000-000000000001",
            "archived": false,
            "created_at": "2026-01-01T00:00:00Z",
        })
    }

    // =========================================================================
    // fetch_stores_sharing_wallet - drives the async function the component
    // actually calls, not just stores_sharing_wallet in isolation, so it
    // catches a wiring bug the pure helper's own tests cannot: whether a
    // failed fetch anywhere in the chain is reported as "unknown" rather than
    // silently read as "no other stores."
    // =========================================================================

    #[test]
    fn reports_the_other_store_sharing_the_current_wallet() {
        let store_id = "22222222-2222-2222-2222-222222222222".to_string();
        let other_id = "33333333-3333-3333-3333-333333333333".to_string();
        let sid = store_id.clone();
        let oid = other_id.clone();
        let transport: TestTransport = Arc::new(move |spec| {
            assert_eq!(spec.method, "GET");
            match spec.path.as_str() {
                p if p == format!("/api/stores/{}/wallet", sid) => {
                    Ok(sample_wallet_json(&sid, "xpub6D4B...eacc"))
                }
                "/api/stores" => Ok(serde_json::json!([
                    sample_store_json(&sid, "This Store"),
                    sample_store_json(&oid, "Coffee Shop"),
                ])),
                p if p == format!("/api/stores/{}/wallet", oid) => {
                    Ok(sample_wallet_json(&oid, "xpub6D4B...eacc"))
                }
                other => panic!("unexpected path {other}"),
            }
        });
        let api = ApiClient::with_test_transport("", transport);

        let sharing = block_on(fetch_stores_sharing_wallet(&api, &store_id));

        assert_eq!(sharing, Some(vec!["Coffee Shop".to_string()]));
    }

    #[test]
    fn reports_none_shared_when_no_other_store_matches() {
        let store_id = "22222222-2222-2222-2222-222222222222".to_string();
        let other_id = "33333333-3333-3333-3333-333333333333".to_string();
        let sid = store_id.clone();
        let oid = other_id.clone();
        let transport: TestTransport = Arc::new(move |spec| match spec.path.as_str() {
            p if p == format!("/api/stores/{}/wallet", sid) => {
                Ok(sample_wallet_json(&sid, "xpub6D4B...eacc"))
            }
            "/api/stores" => Ok(serde_json::json!([
                sample_store_json(&sid, "This Store"),
                sample_store_json(&oid, "Flower Shop"),
            ])),
            p if p == format!("/api/stores/{}/wallet", oid) => {
                Ok(sample_wallet_json(&oid, "xpub6ZZZ...9999"))
            }
            other => panic!("unexpected path {other}"),
        });
        let api = ApiClient::with_test_transport("", transport);

        let sharing = block_on(fetch_stores_sharing_wallet(&api, &store_id));

        assert_eq!(sharing, Some(Vec::new()));
    }

    #[test]
    fn reports_unknown_rather_than_empty_when_the_current_wallet_fetch_fails() {
        let store_id = "22222222-2222-2222-2222-222222222222".to_string();
        let transport: TestTransport = Arc::new(|_| {
            Err(ApiError::Http {
                status: 500,
                message: String::new(),
            })
        });
        let api = ApiClient::with_test_transport("", transport);

        let sharing = block_on(fetch_stores_sharing_wallet(&api, &store_id));

        // This is the case the review flagged: a failed fetch must not read
        // as "Some(vec![])" (checked, nothing found) - that is indistinguishable
        // from a real "no other stores" answer.
        assert_eq!(sharing, None);
    }

    #[test]
    fn reports_unknown_rather_than_a_short_list_when_one_candidate_store_fails() {
        let store_id = "22222222-2222-2222-2222-222222222222".to_string();
        let other_id = "33333333-3333-3333-3333-333333333333".to_string();
        let sid = store_id.clone();
        let oid = other_id.clone();
        let calls = Arc::new(Mutex::new(0u32));
        let calls_clone = calls.clone();
        let transport: TestTransport = Arc::new(move |spec| match spec.path.as_str() {
            p if p == format!("/api/stores/{}/wallet", sid) => {
                Ok(sample_wallet_json(&sid, "xpub6D4B...eacc"))
            }
            "/api/stores" => Ok(serde_json::json!([
                sample_store_json(&sid, "This Store"),
                sample_store_json(&oid, "Coffee Shop"),
            ])),
            p if p == format!("/api/stores/{}/wallet", oid) => {
                *calls_clone.lock().unwrap() += 1;
                Err(ApiError::Network("timeout".to_string()))
            }
            other => panic!("unexpected path {other}"),
        });
        let api = ApiClient::with_test_transport("", transport);

        let sharing = block_on(fetch_stores_sharing_wallet(&api, &store_id));

        assert_eq!(sharing, None);
        // Confirms the failing lookup actually ran, rather than the whole
        // function short-circuiting before it got there for an unrelated reason.
        assert_eq!(*calls.lock().unwrap(), 1);
    }

    // =========================================================================
    // fetch_outstanding_invoice_count - drives the async function the
    // component actually calls, since the pending/processing sum has to
    // read as "unknown" rather than a low number when either fetch fails.
    // =========================================================================

    #[test]
    fn sums_pending_and_processing_across_both_calls() {
        let store_id = "22222222-2222-2222-2222-222222222222".to_string();
        let sid = store_id.clone();
        let transport: TestTransport = Arc::new(move |spec| {
            assert_eq!(spec.method, "GET");
            if spec.path == format!("/api/invoices?store_id={sid}&status=pending&limit=1&offset=0")
            {
                Ok(serde_json::json!({ "total": 3, "invoices": [] }))
            } else if spec.path
                == format!("/api/invoices?store_id={sid}&status=processing&limit=1&offset=0")
            {
                Ok(serde_json::json!({ "total": 2, "invoices": [] }))
            } else {
                panic!("unexpected path {}", spec.path);
            }
        });
        let api = ApiClient::with_test_transport("", transport);

        let count = block_on(fetch_outstanding_invoice_count(&api, &store_id));

        assert_eq!(count, Some(5));
    }

    #[test]
    fn reports_unknown_rather_than_a_low_count_when_a_fetch_fails() {
        let store_id = "22222222-2222-2222-2222-222222222222".to_string();
        let sid = store_id.clone();
        let transport: TestTransport = Arc::new(move |spec| {
            if spec.path == format!("/api/invoices?store_id={sid}&status=pending&limit=1&offset=0")
            {
                Ok(serde_json::json!({ "total": 3, "invoices": [] }))
            } else {
                // The "processing" fetch fails - a regression that read this
                // as "0 processing" would report 3 outstanding instead of
                // "could not check," understating exposure after a
                // compromise.
                Err(ApiError::Network("timeout".to_string()))
            }
        });
        let api = ApiClient::with_test_transport("", transport);

        let count = block_on(fetch_outstanding_invoice_count(&api, &store_id));

        assert_eq!(count, None);
    }

    // =========================================================================
    // rotation_error_message
    // =========================================================================

    #[test]
    fn names_a_pasted_private_key_when_the_server_sent_no_message() {
        let err = ApiError::Http {
            status: 400,
            message: String::new(),
        };
        let message = rotation_error_message(&err, "xprv9s21ZrQH143K3QTDL4LXw2F7HEK3wJUD2nW2nRk4s");
        assert!(message.contains("private key"));
    }

    #[test]
    fn prefers_the_servers_own_message_even_over_a_private_key_guess() {
        let err = ApiError::Http {
            status: 400,
            message: "unsupported namespace".to_string(),
        };
        let message = rotation_error_message(&err, "xprv9s21ZrQH143K3QTDL4LXw2F7HEK3wJUD2nW2nRk4s");
        assert_eq!(message, "unsupported namespace");
    }

    #[test]
    fn falls_back_to_a_generic_malformed_key_message() {
        let err = ApiError::Http {
            status: 400,
            message: String::new(),
        };
        let message = rotation_error_message(&err, "not-a-key-at-all");
        assert!(message.contains("extended public key"));
    }

    #[test]
    fn passes_through_a_non_http_error() {
        let err = ApiError::Network("timeout".to_string());
        let message = rotation_error_message(&err, "xpub6D4B...eacc");
        assert_eq!(message, err.to_string());
    }

    // =========================================================================
    // looks_like_a_private_key
    // =========================================================================

    #[test]
    fn detects_mainnet_and_testnet_private_key_prefixes() {
        assert!(looks_like_a_private_key(
            "xprv9s21ZrQH143K3QTDL4LXw2F7HEK3wJUD2nW2nRk4stbPy6cq4o..."
        ));
        assert!(looks_like_a_private_key("tprv8ZgxMBicQKsPd..."));
    }

    #[test]
    fn accepts_leading_and_trailing_whitespace() {
        assert!(looks_like_a_private_key(
            "  xprv9s21ZrQH143K3QTDL4LXw2F7HEK3wJUD2nW2nRk4stbPy6cq4o...  "
        ));
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
        let sharing = stores_sharing_wallet("xpub6D4B...eacc", "store-1", resolved.into_iter());
        assert_eq!(
            sharing,
            vec!["Coffee Shop".to_string(), "Book Store".to_string()]
        );
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
