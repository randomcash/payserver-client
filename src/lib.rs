//! PayServer Web Client
//!
//! Leptos-based frontend for the random.cash payservers, providing:
//! - Invoice management
//! - Payment monitoring
//! - Store configuration
//! - Wallet management
//!
//! Can run standalone or be loaded as a module in the dashboard aggregator.

#![allow(clippy::items_after_test_module)]

pub mod api;
pub mod app;
pub mod components;
pub mod pages;
pub mod services;
pub mod telemetry;
pub mod util;

pub use app::App;

use types::ChainId;
use ui_kit::module::{CheckoutPluginConfig, ModuleInfo};
use ui_kit::types::RouteInfo;
use wasm_bindgen::prelude::*;

/// EVM module identifier.
pub const MODULE_ID: &str = "evm";

/// EVM module display name.
pub const MODULE_NAME: &str = "Ethereum";

/// Get module info for the dashboard aggregator.
pub fn module_info(api_url: String) -> ModuleInfo {
    ModuleInfo {
        id: MODULE_ID,
        name: MODULE_NAME,
        icon: "ethereum",
        api_url,
        routes: vec![
            RouteInfo {
                path: "/evm".to_string(),
                label: "Dashboard".to_string(),
                icon: Some("dashboard".to_string()),
            },
            RouteInfo {
                path: "/evm/invoices".to_string(),
                label: "Invoices".to_string(),
                icon: Some("receipt".to_string()),
            },
            RouteInfo {
                path: "/evm/payments".to_string(),
                label: "Payments".to_string(),
                icon: Some("payments".to_string()),
            },
            RouteInfo {
                path: "/evm/stores".to_string(),
                label: "Stores".to_string(),
                icon: Some("store".to_string()),
            },
            RouteInfo {
                path: "/evm/wallets".to_string(),
                label: "Wallets".to_string(),
                icon: Some("wallet".to_string()),
            },
            RouteInfo {
                path: "/evm/settings".to_string(),
                label: "Settings".to_string(),
                icon: Some("settings".to_string()),
            },
        ],
    }
}

/// Get checkout plugin configuration for the checkout page.
pub fn checkout_plugin() -> CheckoutPluginConfig {
    CheckoutPluginConfig {
        module_id: MODULE_ID,
        network_badge: render_network_badge,
        amount_details: Some(render_amount_details),
        qr_code: render_qr_code,
        wallet_actions: Some(render_wallet_actions),
    }
}

/// The invoice details `wallet_actions` needs but cannot receive as an
/// argument.
///
/// `ui_kit::module::checkout_slots::WalletActionsFn` is `fn(payment_address,
/// chain_id) -> Option<AnyView>` — frozen in `payserver-commons`, pinned by
/// `rev`, so widening it for one caller means the three-step cross-repo dance
/// for a single-ticket change. The amount and token travel through context
/// instead, provided by `checkout.rs` right before it calls the slot.
#[derive(Clone)]
pub(crate) struct WalletActionsContext {
    pub amount: String,
    pub token_address: Option<String>,
}

/// Outcome of a connect-and-send attempt, shown under the wallet list.
#[derive(Clone)]
enum WalletActionStatus {
    Idle,
    Connecting,
    WrongNetwork(String),
    Sent,
    Error(String),
}

// Checkout slot implementations
fn render_network_badge(chain_id: &ChainId, network_name: &str) -> leptos::prelude::AnyView {
    use leptos::prelude::*;
    use ui_kit::components::crypto::NetworkBadge;

    // The name is passed through rather than derived: a CAIP-2 reference is
    // mostly an opaque genesis hash, so nothing can turn one into a name.
    view! {
        <NetworkBadge chain_id=chain_id.clone() name=network_name.to_string() />
    }
    .into_any()
}

fn render_amount_details(chain_id: &ChainId, _amount: &str) -> Option<leptos::prelude::AnyView> {
    use leptos::prelude::*;

    // Show gas info for EVM chains
    Some(
        view! {
            <div class="evm-amount-details">
                <span class="evm-chain-info">"Chain: " {chain_id.to_string()}</span>
            </div>
        }
        .into_any(),
    )
}

fn render_qr_code(payment_request: &str) -> leptos::prelude::AnyView {
    use leptos::prelude::*;
    use ui_kit::components::crypto::QrCodeCard;

    let data = payment_request.to_string();

    view! {
        <QrCodeCard data=data label="Scan to pay" size=250 />
    }
    .into_any()
}

/// EIP-6963 wallet connect. Renders nothing until a wallet actually answers
/// discovery — an empty list is the normal "no wallet installed" case, not an
/// error, and the copy-address/QR path above this is the primary route
/// regardless of what happens here. This slot only reads `payment_address`
/// and the invoice amount/token from [`WalletActionsContext`]; it never
/// touches the signals that drive the displayed address or amount.
fn render_wallet_actions(
    payment_address: &str,
    chain_id: &ChainId,
) -> Option<leptos::prelude::AnyView> {
    use leptos::prelude::*;

    use crate::services::eip6963::discover_wallets;

    if !chain_id.is_evm() {
        return None;
    }

    // Not provided means whoever called this slot isn't checkout's payment
    // flow (or forgot to set it up) — degrade to nothing rather than a
    // wallet-connect UI with no idea what to transfer.
    let transfer = use_context::<WalletActionsContext>()?;

    let payment_address = payment_address.to_string();
    let chain_id = chain_id.clone();
    let expected_chain_hex = chain_id.evm_chain_id().map(|id| format!("0x{id:x}"));
    let chain_label = crate::util::chain_name(&chain_id).to_string();

    let wallets = discover_wallets();
    let (status, set_status) = signal(WalletActionStatus::Idle);

    Some(
        view! {
            <Show when=move || !wallets.get().is_empty()>
                <div class="checkout-wallet-actions">
                    <span class="checkout-wallet-label">"Or pay with a connected wallet"</span>
                    <div class="checkout-wallet-list">
                        {
                            // Cloned here, outside the reactive closure: `Show`
                            // and this dynamic child both need `Fn`, callable
                            // more than once, and a closure that *moves* its
                            // own captured field into a nested closure can
                            // only be `FnOnce`. Cloning first means each call
                            // borrows the captured original and moves a fresh
                            // copy onward instead.
                            let payment_address = payment_address.clone();
                            let expected_chain_hex = expected_chain_hex.clone();
                            let chain_label = chain_label.clone();
                            let transfer = transfer.clone();
                            move || {
                                let payment_address = payment_address.clone();
                                let expected_chain_hex = expected_chain_hex.clone();
                                let chain_label = chain_label.clone();
                                let transfer = transfer.clone();
                                wallets.get().into_iter().map(move |wallet| {
                                    let payment_address = payment_address.clone();
                                    let expected_chain_hex = expected_chain_hex.clone();
                                    let chain_label = chain_label.clone();
                                    let transfer = transfer.clone();
                                    let provider = wallet.provider.clone();
                                    view! {
                                        <button
                                            class="ps-btn ps-btn-secondary checkout-wallet-option"
                                            on:click=move |_| {
                                                let provider = provider.clone();
                                                let payment_address = payment_address.clone();
                                                let expected_chain_hex = expected_chain_hex.clone();
                                                let chain_label = chain_label.clone();
                                                let transfer = transfer.clone();
                                                set_status.set(WalletActionStatus::Connecting);
                                                leptos::task::spawn_local(async move {
                                                    let outcome = connect_and_send(
                                                        &provider,
                                                        &payment_address,
                                                        expected_chain_hex.as_deref(),
                                                        &chain_label,
                                                        &transfer,
                                                    )
                                                    .await;
                                                    set_status.set(outcome);
                                                });
                                            }
                                        >
                                            {wallet.name.clone()}
                                        </button>
                                    }
                                }).collect_view()
                            }
                        }
                    </div>
                    {move || match status.get() {
                        WalletActionStatus::Idle => None,
                        WalletActionStatus::Connecting => Some(view! {
                            <p class="checkout-wallet-status">"Connecting..."</p>
                        }.into_any()),
                        WalletActionStatus::WrongNetwork(chain) => Some(view! {
                            <p class="checkout-wallet-status checkout-wallet-error">
                                {format!("Wrong network — switch to {chain} in your wallet")}
                            </p>
                        }.into_any()),
                        WalletActionStatus::Sent => Some(view! {
                            <p class="checkout-wallet-status checkout-wallet-success">
                                "Transaction sent — waiting for confirmation."
                            </p>
                        }.into_any()),
                        WalletActionStatus::Error(message) => Some(view! {
                            <p class="checkout-wallet-status checkout-wallet-error">{message}</p>
                        }.into_any()),
                    }}
                </div>
            </Show>
        }
        .into_any(),
    )
}

/// Connect to one announced provider and send a transfer prefilled from the
/// invoice: chain checked, recipient and base-unit amount taken from the
/// checkout data, never from anything the wallet reports.
async fn connect_and_send(
    provider: &wasm_bindgen::JsValue,
    payment_address: &str,
    expected_chain_hex: Option<&str>,
    chain_label: &str,
    transfer: &WalletActionsContext,
) -> WalletActionStatus {
    use crate::services::eip6963::provider_request;

    let accounts = match provider_request(
        provider,
        "eth_requestAccounts",
        &js_sys::Array::new().into(),
    )
    .await
    {
        Ok(v) => v,
        Err(e) => return WalletActionStatus::Error(e),
    };
    if js_sys::Array::from(&accounts).length() == 0 {
        return WalletActionStatus::Error("wallet returned no account".to_string());
    }

    if let Some(expected) = expected_chain_hex {
        let reported =
            match provider_request(provider, "eth_chainId", &js_sys::Array::new().into()).await {
                Ok(v) => v,
                Err(e) => return WalletActionStatus::Error(e),
            };
        let matches = reported
            .as_string()
            .is_some_and(|r| r.eq_ignore_ascii_case(expected));
        if !matches {
            return WalletActionStatus::WrongNetwork(chain_label.to_string());
        }
    }

    let tx = js_sys::Object::new();
    match &transfer.token_address {
        Some(token) => {
            let calldata = match erc20_transfer_calldata(payment_address, &transfer.amount) {
                Ok(data) => data,
                Err(e) => return WalletActionStatus::Error(e),
            };
            if js_sys::Reflect::set(&tx, &"to".into(), &token.as_str().into()).is_err()
                || js_sys::Reflect::set(&tx, &"data".into(), &calldata.into()).is_err()
            {
                return WalletActionStatus::Error("failed to build transaction".to_string());
            }
        }
        None => {
            let value_hex = match transfer.amount.parse::<u128>() {
                Ok(amount) => format!("0x{amount:x}"),
                Err(_) => return WalletActionStatus::Error("malformed invoice amount".to_string()),
            };
            if js_sys::Reflect::set(&tx, &"to".into(), &payment_address.into()).is_err()
                || js_sys::Reflect::set(&tx, &"value".into(), &value_hex.into()).is_err()
            {
                return WalletActionStatus::Error("failed to build transaction".to_string());
            }
        }
    }

    let params = js_sys::Array::new();
    params.push(&tx);

    match provider_request(provider, "eth_sendTransaction", &params.into()).await {
        Ok(_) => WalletActionStatus::Sent,
        Err(e) => WalletActionStatus::Error(e),
    }
}

/// ABI-encoded `transfer(address,uint256)` call — the selector is the first
/// four bytes of `keccak256("transfer(address,uint256)")`, `0xa9059cbb`, a
/// fixed constant rather than something to hash at runtime.
fn erc20_transfer_calldata(recipient: &str, amount_base_units: &str) -> Result<String, String> {
    let recipient_hex = recipient
        .strip_prefix("0x")
        .filter(|h| h.len() == 40 && h.bytes().all(|b| b.is_ascii_hexdigit()))
        .ok_or_else(|| "malformed recipient address".to_string())?;
    let amount: u128 = amount_base_units
        .parse()
        .map_err(|_| "malformed invoice amount".to_string())?;

    Ok(format!(
        "0xa9059cbb{:0>64}{amount:064x}",
        recipient_hex.to_lowercase()
    ))
}

#[cfg(test)]
mod wallet_action_tests {
    use super::*;

    const RECIPIENT: &str = "0x66da354a361225a8C4FF5232f413A80af943C14c";

    #[test]
    fn erc20_calldata_targets_the_selector_and_pads_address_and_amount_to_32_bytes() {
        let calldata = erc20_transfer_calldata(RECIPIENT, "1000000").expect("calldata");
        assert_eq!(
            calldata,
            "0xa9059cbb\
             00000000000000000000000066da354a361225a8c4ff5232f413a80af943c14c\
             00000000000000000000000000000000000000000000000000000000000f4240"
        );
    }

    /// A masked or truncated address looks plausible but points nowhere — the
    /// same mistake `payment_request_uri` in `types` guards against, and just
    /// as costly here since this calldata is what the wallet actually sends.
    #[test]
    fn a_malformed_recipient_is_refused() {
        assert!(erc20_transfer_calldata("not-an-address", "1000000").is_err());
        assert!(erc20_transfer_calldata("0x1234", "1000000").is_err());
        assert!(erc20_transfer_calldata("", "1000000").is_err());
    }

    /// Base units only, like the EIP-681 URI path — a decimal amount here
    /// would encode the wrong quantity into a transaction a customer is about
    /// to sign.
    #[test]
    fn a_non_integer_amount_is_refused() {
        assert!(erc20_transfer_calldata(RECIPIENT, "0.047").is_err());
        assert!(erc20_transfer_calldata(RECIPIENT, "").is_err());
    }
}

/// Mount the app into `#app`, clearing whatever placeholder is there first.
///
/// This is the single mounting implementation. Trunk builds the cdylib
/// (`data-target-name="payserver_client"` in index.html), so `init` below is
/// the entry point that actually ships; `src/main.rs` exists for the standalone
/// bin target. Both call this, so a fix to one cannot silently miss the other —
/// which is exactly how the first-paint loader survived its own removal code.
pub fn mount_app() {
    use leptos::mount::mount_to;
    use leptos::prelude::*;
    use wasm_bindgen::JsCast;

    console_error_panic_hook::set_once();
    // Forward panics / uncaught JS errors to errex. A no-op unless the
    // `errex-dsn` meta tag in index.html is filled in — see `telemetry`.
    telemetry::init();
    web_sys::console::log_1(&"EVM PayServer: Starting app...".into());

    // Mount to the #app div
    let app_element = document()
        .get_element_by_id("app")
        .expect("Could not find #app element")
        .unchecked_into::<web_sys::HtmlElement>();

    web_sys::console::log_1(&"EVM PayServer: Found #app element, mounting...".into());

    // Remove the inline first-paint loader (#initial-loader in index.html).
    // Leptos `mount_to` appends to #app rather than replacing its contents, so
    // the placeholder must be cleared explicitly or it covers the mounted app
    // forever (it is position:fixed; inset:0).
    app_element.set_inner_html("");

    mount_to(app_element, App).forget();

    web_sys::console::log_1(&"EVM PayServer: App mounted successfully".into());
}

/// Initialize and mount the app (called when loaded as WASM module).
#[wasm_bindgen(start)]
pub fn init() {
    mount_app();
}
