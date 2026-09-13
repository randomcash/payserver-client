//! Public checkout page for customer payments.
//!
//! Displayed at `/checkout/{invoice_id}`. No authentication required.
//! Shows invoice amount, QR code, chain/asset selector, countdown timer,
//! and real-time payment status via WebSocket.

use leptos::prelude::*;
use leptos_router::hooks::use_params_map;

use send_wrapper::SendWrapper;

use ui_kit::CopyButton;

use crate::api::{ApiClient, ApiError, CheckoutResponse, PaymentOption};
use crate::services::websocket::{StatusUpdate, WebSocketService};
use crate::util::chain_name;

mod countdown;
mod qr_picker;
use countdown::CountdownTimer;
use qr_picker::{QrEncoding, QrPicker};

/// How often the page re-reads the invoice when nothing has pushed to it.
///
/// Slow enough to be free - six requests a minute against a read budget of
/// sixty - and fast enough that a customer who has paid is never left looking
/// at a stale countdown for long.
const POLL_INTERVAL_MS: u32 = 10_000;

/// Format a human-readable amount from smallest units.
///
/// e.g., "1000000" with 6 decimals -> "1.000000"
fn format_crypto_amount(smallest_units: &str, decimals: u8) -> String {
    if decimals == 0 {
        return smallest_units.to_string();
    }
    let s = smallest_units.to_string();
    let len = s.len();
    let d = decimals as usize;
    if len <= d {
        let zeros = "0".repeat(d - len);
        format!("0.{}{}", zeros, s.trim_end_matches('0'))
    } else {
        let (int_part, frac_part) = s.split_at(len - d);
        let trimmed = frac_part.trim_end_matches('0');
        if trimmed.is_empty() {
            int_part.to_string()
        } else {
            format!("{}.{}", int_part, trimmed)
        }
    }
}

/// Public checkout page.
#[component]
pub fn CheckoutPage() -> impl IntoView {
    let params = use_params_map();
    let invoice_id = move || params.get().get("id").unwrap_or_default();

    // Create unauthenticated API client (same-origin, no auth header).
    let api = ApiClient::unauthenticated();

    // Selected payment option index
    let (selected_idx, set_selected_idx) = signal(0usize);

    // Clipboard feedback

    // Refresh counter for manual retry / WS-triggered refresh
    let (refresh, set_refresh) = signal(0u32);

    let checkout_resource = LocalResource::new(move || {
        let api = api.clone();
        let id = invoice_id();
        let _ = refresh.get();
        async move {
            if id.is_empty() {
                return Err(ApiError::Network("No invoice ID".to_string()));
            }
            api.get_checkout(&id).await
        }
    });

    // WebSocket for real-time updates
    let ws = std::rc::Rc::new(WebSocketService::new());
    let ws_state = ws.connection_state();
    let ws_update = ws.last_update();

    // Connect WebSocket reactively — reconnects when invoice_id changes
    // (e.g. navigating from /checkout/A to /checkout/B without unmounting).
    let ws_for_effect = ws.clone();
    Effect::new(move |_| {
        let id = invoice_id();
        if id.is_empty() {
            ws_for_effect.disconnect();
            return;
        }
        let protocol = if web_sys::window()
            .and_then(|w| w.location().protocol().ok())
            .as_deref()
            == Some("https:")
        {
            "wss"
        } else {
            "ws"
        };
        let host = web_sys::window()
            .and_then(|w| w.location().host().ok())
            .unwrap_or_default();
        let ws_url = format!(
            "{}://{}/api/checkout/ws?invoice_id={}",
            protocol,
            host,
            js_sys::encode_uri_component(&id)
        );
        let _ = ws_for_effect.connect(&ws_url, None);
        // Reset refresh so the resource re-fetches with fresh WS state
        set_refresh.update(|n| *n += 1);
    });

    // A poll behind the socket, because the socket is the fast path and not
    // the source of truth.
    //
    // Without this the page refetched *only* on a WebSocket message, so a
    // socket that never connected meant a customer who had paid watched the
    // countdown run out on an invoice the server had already marked paid.
    // That is what happened: the `ws` rate limit refused the sixth connection
    // attempt in a minute, the refusal closed the socket, the close scheduled
    // another attempt, and the loop sustained itself. The payment had settled
    // six minutes earlier.
    //
    // Ten seconds costs six requests a minute against a read budget of sixty,
    // and it bounds the worst case at "ten seconds late" instead of "never".
    // The socket still does the work when it is up; this only decides what
    // happens when it is not.
    let poll_handle: std::rc::Rc<std::cell::RefCell<Option<gloo_timers::callback::Interval>>> =
        std::rc::Rc::new(std::cell::RefCell::new(None));
    let poll_for_effect = poll_handle.clone();
    Effect::new(move |_| {
        if invoice_id().is_empty() {
            *poll_for_effect.borrow_mut() = None;
            return;
        }
        let interval = gloo_timers::callback::Interval::new(POLL_INTERVAL_MS, move || {
            set_refresh.update(|n| *n += 1);
        });
        *poll_for_effect.borrow_mut() = Some(interval);
    });

    let poll_cleanup = SendWrapper::new(poll_handle);
    on_cleanup(move || {
        *poll_cleanup.borrow_mut() = None;
    });

    // Clean up WebSocket on unmount
    let ws_cleanup = SendWrapper::new(ws.clone());
    on_cleanup(move || {
        ws_cleanup.disconnect();
    });

    // Refresh data when a relevant WS update arrives
    Effect::new(move || {
        if let Some(StatusUpdate::InvoiceStatus { .. } | StatusUpdate::PaymentUpdate { .. }) =
            ws_update.get()
        {
            set_refresh.update(|n| *n += 1);
        }
    });

    view! {
        <div class="checkout-page">
            <div class="checkout-container">
                <div class="checkout-header">
                    <span class="checkout-logo">"E"</span>
                    <span class="checkout-brand">"ETHPayServer"</span>
                </div>

                <Suspense fallback=move || view! {
                    <div class="checkout-loading">
                        <div class="loading-spinner"></div>
                        <p>"Loading checkout..."</p>
                    </div>
                }>
                    {move || checkout_resource.get().map(|result| match &*result {
                        Err(_) => view! {
                            <div class="checkout-error">
                                <h2>"Invoice unavailable"</h2>
                                <p>"This invoice could not be loaded. Check the link and try again."</p>
                            </div>
                        }.into_any(),
                        Ok(data) => {
                            let data = data.clone();
                            render_checkout(data, selected_idx, set_selected_idx)
                        }
                    })}
                </Suspense>

                <div class="checkout-footer">
                    <span class="checkout-ws-status">
                        {move || match ws_state.get() {
                            crate::services::websocket::ConnectionState::Connected => "Live",
                            _ => "",
                        }}
                    </span>
                    <span class="checkout-powered">"Powered by ETHPayServer"</span>
                </div>
            </div>
        </div>
    }
}

/// Render the checkout content once data is loaded.
fn render_checkout(
    data: CheckoutResponse,
    selected_idx: ReadSignal<usize>,
    set_selected_idx: WriteSignal<usize>,
) -> AnyView {
    let status = data.status.clone();

    // Terminal states
    if data.is_paid {
        return view! {
            <div class="checkout-status checkout-paid">
                <div class="checkout-status-icon">"&#10003;"</div>
                <h2>"Payment Complete"</h2>
                <p class="checkout-amount">{data.amount.clone()}" "{data.currency.clone()}</p>
                <p class="checkout-status-detail">"Thank you for your payment."</p>
            </div>
        }
        .into_any();
    }

    if data.is_expired {
        return view! {
            <div class="checkout-status checkout-expired">
                <div class="checkout-status-icon">"&#10007;"</div>
                <h2>"Invoice Expired"</h2>
                <p class="checkout-amount">{data.amount.clone()}" "{data.currency.clone()}</p>
                <p class="checkout-status-detail">"This invoice is no longer accepting payments."</p>
            </div>
        }
        .into_any();
    }

    if status == "cancelled" {
        return view! {
            <div class="checkout-status checkout-expired">
                <div class="checkout-status-icon">"&#10007;"</div>
                <h2>"Invoice Cancelled"</h2>
                <p class="checkout-amount">{data.amount.clone()}" "{data.currency.clone()}</p>
            </div>
        }
        .into_any();
    }

    // Active payment states
    let active_options: Vec<PaymentOption> = data
        .payment_options
        .iter()
        .filter(|o| o.is_active)
        .cloned()
        .collect();

    if active_options.is_empty() {
        return view! {
            <div class="checkout-status checkout-expired">
                <h2>"No Payment Options"</h2>
                <p>"This invoice has no active payment methods configured."</p>
            </div>
        }
        .into_any();
    }

    let options = active_options.clone();
    let options_for_selector = active_options.clone();
    let amount = data.amount.clone();
    let currency = data.currency.clone();
    let expires_at = data.expires_at;

    let status_label = match status.as_str() {
        "processing" => "Payment detected, awaiting confirmation...",
        "partially_paid" => "Partial payment received",
        "late_paid" => "Payment received late — awaiting merchant review",
        "refunded" => "Payment refunded",
        _ => "Awaiting payment",
    };

    view! {
        <div class="checkout-body">
            // Amount and status
            <div class="checkout-amount-section">
                <p class="checkout-amount">{amount.clone()}" "{currency.clone()}</p>
                <p class="checkout-status-label">{status_label}</p>
                <CountdownTimer expires_at=expires_at.to_rfc3339() />
            </div>

            // Chain/asset selector
            {if options_for_selector.len() > 1 {
                let opts = options_for_selector.clone();
                Some(view! {
                    <div class="checkout-chain-selector">
                        <label>"Pay with"</label>
                        <div class="checkout-chain-options">
                            {opts.into_iter().enumerate().map(|(i, opt)| {
                                let label = format!("{} ({})", opt.asset_symbol, chain_name(&opt.chain_id));
                                view! {
                                    <button
                                        class=move || if selected_idx.get() == i { "chain-option active" } else { "chain-option" }
                                        on:click=move |_| set_selected_idx.set(i)
                                    >
                                        {label}
                                    </button>
                                }
                            }).collect_view()}
                        </div>
                    </div>
                })
            } else {
                None
            }}

            // QR code and payment details
            {move || {
                let idx = selected_idx.get();
                let opt = options.get(idx).or(options.first());
                opt.map(|option| {
                    let addr = option.payment_address.clone();
                    let display_amount = format_crypto_amount(&option.amount, option.decimals);
                    let asset = option.asset_symbol.clone();
                    let chain = chain_name(&option.chain_id).to_string();
                    let addr_for_copy = addr.clone();

                    // Which apps can use this QR depends on what the customer is
                    // paying from, and neither encoding works for everyone: a
                    // wallet wants EIP-681 (it prefills amount and chain, the
                    // one time in the flow a typo costs money); an exchange
                    // withdrawal form takes only a bare address and errors on
                    // anything else. So both are offered as separate cards
                    // rather than picking one - EIP-681 first, since a wallet
                    // is the common case.
                    //
                    // `option.amount` is already base units - the same string
                    // `format_crypto_amount` divides for display - which is what
                    // the URI wants. Nothing converts through a float.
                    //
                    // The EIP-681 card is omitted, not emptied, when a URI
                    // cannot be built with certainty (a non-EVM chain, an
                    // address that does not parse): a card with no URI must not
                    // appear, since a wallet misparsing one can send the wrong
                    // amount to the wrong place, where a bare address always
                    // lets the customer proceed by hand.
                    let mut encodings = Vec::with_capacity(2);
                    if let Some(uri) = types::payment_request_uri(
                        &option.chain_id,
                        &addr,
                        &option.amount,
                        option.token_address.as_deref(),
                    ) {
                        encodings.push(QrEncoding {
                            label: "Scan with a wallet",
                            data: uri,
                        });
                    }
                    encodings.push(QrEncoding {
                        label: "Copy to an exchange withdrawal",
                        data: addr.clone(),
                    });

                    // The `wallet_actions` slot signature is fixed at
                    // (address, chain_id) — the amount travels through
                    // context instead, read back inside the slot
                    // implementation. See `WalletActionsContext`.
                    provide_context(crate::WalletActionsContext {
                        amount: option.amount.clone(),
                        token_address: option.token_address.clone(),
                    });
                    let wallet_actions = (crate::checkout_plugin().wallet_actions)
                        .and_then(|render| render(&addr, &option.chain_id));

                    view! {
                        <div class="checkout-payment-details">
                            <div class="checkout-qr">
                                <QrPicker encodings=encodings />
                            </div>

                            // Amount in crypto
                            <div class="checkout-detail-row">
                                <span class="checkout-detail-label">"Amount"</span>
                                <span class="checkout-detail-value">{display_amount.clone()}" "{asset.clone()}</span>
                            </div>

                            // Network
                            <div class="checkout-detail-row">
                                <span class="checkout-detail-label">"Network"</span>
                                <span class="checkout-detail-value">{chain}</span>
                            </div>

                            // Address with copy
                            <div class="checkout-address-row">
                                <span class="checkout-detail-label">"Address"</span>
                                <div class="checkout-address-value">
                                    <code class="checkout-address">{addr.clone()}</code>
                                    // The payer is about to send money to whatever is on their
                                    // clipboard. This reported success without waiting for the
                                    // clipboard to accept the write - and this is the page most
                                    // likely to be opened in an in-app browser, where it is refused.
                                    <CopyButton text=addr_for_copy.clone() />
                                </div>
                            </div>

                            {wallet_actions}
                        </div>
                    }
                })
            }}
        </div>
    }
    .into_any()
}
