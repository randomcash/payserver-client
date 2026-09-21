//! Invoice detail page and its inner content component.

use leptos::prelude::*;
use leptos_router::components::A;
use leptos_router::hooks::use_params_map;

use ui_kit::CopyButton;

use crate::api::{ApiClient, ApiError, Invoice, InvoiceStatusExt, Payment};
use crate::components::{TimelineState, payment_state};

use super::helpers::{
    IconExport, chain_name, confirmed_payment_count, format_amount, format_date,
    get_metadata_field, payment_status, payment_status_class, truncate_hex,
};

/// The customer-facing checkout URL for an invoice.
///
/// Not the current (admin) URL - a merchant copying this link is sending it
/// to whoever owes them money, so it has to be the public one regardless of
/// where in the dashboard the button was clicked.
fn checkout_url(invoice_id: &str) -> String {
    let origin = web_sys::window()
        .and_then(|w| w.location().origin().ok())
        .unwrap_or_default();
    format!(
        "{origin}/checkout/{}",
        js_sys::encode_uri_component(invoice_id)
    )
}

/// Escape a field for CSV output: quote it if it holds a comma, quote or
/// newline, doubling any quotes inside.
fn csv_escape(field: &str) -> String {
    if field.contains(',') || field.contains('"') || field.contains('\n') || field.contains('\r') {
        format!("\"{}\"", field.replace('"', "\"\""))
    } else {
        field.to_string()
    }
}

/// Build one CRLF-terminated CSV row.
fn csv_row(fields: &[&str]) -> String {
    let mut row = fields
        .iter()
        .map(|f| csv_escape(f))
        .collect::<Vec<_>>()
        .join(",");
    row.push_str("\r\n");
    row
}

/// Render an invoice and its payments as CSV, for the "Download" action.
///
/// Client-side, from data the page already has - the server holds no
/// per-invoice export today, and this is enough to be the merchant's record
/// of a single invoice without a new endpoint.
fn invoice_csv(invoice: &Invoice, payments: &[Payment]) -> String {
    let order_id = get_metadata_field(invoice, "order_id").unwrap_or_default();
    let buyer_email = get_metadata_field(invoice, "buyer_email").unwrap_or_default();

    let mut content = csv_row(&[
        "id",
        "store_id",
        "status",
        "currency",
        "amount",
        "amount_received",
        "created_at",
        "expires_at",
        "order_id",
        "customer_email",
    ]);
    content.push_str(&csv_row(&[
        &invoice.id,
        &invoice.store_id,
        invoice.status.label(),
        &invoice.currency,
        &invoice.amount,
        &invoice.amount_received,
        &invoice.created_at.to_rfc3339(),
        &invoice.expires_at.to_rfc3339(),
        &order_id,
        &buyer_email,
    ]));

    if !payments.is_empty() {
        content.push_str("\r\n");
        content.push_str(&csv_row(&[
            "payment_tx_hash",
            "asset_symbol",
            "amount",
            "status",
            "from_address",
            "detected_at",
            "confirmed_at",
        ]));
        for p in payments {
            let from = p.from_address.clone().unwrap_or_default();
            let confirmed = p.confirmed_at.map(|t| t.to_rfc3339()).unwrap_or_default();
            content.push_str(&csv_row(&[
                &p.tx_hash,
                &p.asset_symbol,
                &p.amount,
                payment_status(p),
                &from,
                &p.detected_at.to_rfc3339(),
                &confirmed,
            ]));
        }
    }

    content
}

/// Invoice detail page - fetches from GET /invoices/{id} and GET /invoices/{id}/payments.
#[component]
pub fn InvoiceDetailPage() -> impl IntoView {
    let params = use_params_map();
    let api = use_context::<Signal<ApiClient>>().expect("ApiClient must be provided");

    let invoice_id = Signal::derive(move || params.get().get("id").unwrap_or_default());

    // Refresh counter for retry on error
    let (refresh, set_refresh) = signal(0u32);

    let detail_resource = LocalResource::new(move || {
        let api = api.get();
        let id = invoice_id.get();
        let _ = refresh.get();
        async move {
            if id.is_empty() {
                return Err(ApiError::Network("No invoice ID".to_string()));
            }
            let invoice = api.get_invoice(&id).await?;
            let payments = api.get_invoice_payments(&id).await?;
            Ok((invoice, payments))
        }
    });

    view! {
        <div class="invoice-detail-page">
            <Suspense fallback=move || view! {
                <div class="loading-container">
                    <div class="loading-spinner"></div>
                    <p>"Loading invoice..."</p>
                </div>
            }>
                {move || detail_resource.get().map(|result| match &*result {
                    Err(e) => view! {
                        <div class="error-container">
                            <p class="error-message">{e.to_string()}</p>
                            <button class="ps-btn ps-btn-secondary ps-btn-sm" on:click=move |_| set_refresh.update(|n| *n += 1)>
                                "Retry"
                            </button>
                        </div>
                    }.into_any(),
                    Ok((invoice, payments)) => {
                        view! {
                            <InvoiceDetailContent invoice=invoice.clone() payments=payments.clone() />
                        }.into_any()
                    }
                })}
            </Suspense>
        </div>
    }
}

/// Inner content of the invoice detail page (rendered after data loads).
#[component]
fn InvoiceDetailContent(invoice: Invoice, payments: Vec<Payment>) -> impl IntoView {
    let order_id = get_metadata_field(&invoice, "order_id");
    let buyer_email = get_metadata_field(&invoice, "buyer_email");
    let created_display = format_date(&invoice.created_at.to_rfc3339());
    let expires_display = format_date(&invoice.expires_at.to_rfc3339());
    let confirmed_count = confirmed_payment_count(&payments);
    let payment_count = payments.len();
    let is_paid = invoice.status == types::InvoiceStatus::Paid;
    let is_expired = invoice.status == types::InvoiceStatus::Expired;

    // Customer avatar: first letter of email, or "?"
    let avatar_letter = buyer_email
        .as_ref()
        .and_then(|e| e.chars().next())
        .map(|c| c.to_uppercase().to_string())
        .unwrap_or_else(|| "?".to_string());
    let email_display = buyer_email
        .clone()
        .unwrap_or_else(|| "No email".to_string());

    view! {
        // Title row
        <div class="invoice-detail-header">
            <div class="invoice-detail-header-left">
                <A href="/evm/invoices" attr:class="back-link">
                    <IconArrowLeft />
                    "Invoices"
                </A>
                <div class="invoice-detail-title-row">
                    <h1 class="invoice-detail-title">{invoice.id.clone()}</h1>
                    <span class=invoice.status.css_class()>{invoice.status.label()}</span>
                </div>
                {order_id.clone().map(|oid| view! {
                    <p class="invoice-detail-subtitle">"Order: "{oid}</p>
                })}
            </div>
            <div class="invoice-detail-actions">
                <CopyButton
                    text=checkout_url(&invoice.id)
                    label="Copy link"
                    class="ps-btn-sm"
                />
                <button
                    class="ps-btn ps-btn-secondary ps-btn-sm"
                    on:click={
                        let invoice = invoice.clone();
                        let payments = payments.clone();
                        move |_| {
                            let csv = invoice_csv(&invoice, &payments);
                            let filename = format!("invoice-{}.csv", invoice.id);
                            crate::pages::trigger_csv_download(&csv, &filename);
                        }
                    }
                >
                    <IconExport />
                    "Download"
                </button>
            </div>
        </div>

        // Content
        <div class="invoice-detail-content">
            // Main Info Cards
            <div class="invoice-detail-main">
                // Invoice Summary Card
                <div class="ps-card">
                    <div class="ps-card-header">
                        <h3>"Invoice details"</h3>
                    </div>
                    <div class="ps-card-body">
                        <div class="detail-row">
                            <span class="detail-label">"Amount due"</span>
                            <span class="detail-value detail-value-lg">
                                {format_amount(&invoice.amount, &invoice.currency)}
                            </span>
                        </div>
                        <div class="detail-row">
                            <span class="detail-label">"Amount received"</span>
                            <span class="detail-value detail-value-success">
                                {format_amount(&invoice.amount_received, &invoice.currency)}
                            </span>
                        </div>
                        <div class="detail-row">
                            <span class="detail-label">"Payments"</span>
                            <span class="detail-value">
                                {format!("{} confirmed / {} total", confirmed_count, payment_count)}
                            </span>
                        </div>
                        <div class="detail-row">
                            <span class="detail-label">"Created"</span>
                            <span class="detail-value">{created_display.clone()}</span>
                        </div>
                        <div class="detail-row">
                            <span class="detail-label">"Expires"</span>
                            <span class="detail-value">{expires_display}</span>
                        </div>
                    </div>
                </div>

                // Payments Card
                <div class="ps-card">
                    <div class="ps-card-header">
                        <h3>"Payments"</h3>
                        <span class="payment-count">{payment_count}" payment(s)"</span>
                    </div>
                    <div class="ps-card-body payments-body">
                        {if payments.is_empty() {
                            view! {
                                <div class="payments-empty">
                                    <IconClock />
                                    <span>"No payments received yet"</span>
                                </div>
                            }.into_any()
                        } else {
                            view! {
                                <div class="payments-table-container">
                                    <table class="payments-table">
                                        <thead>
                                            <tr>
                                                <th>"Transaction"</th>
                                                <th>"Asset"</th>
                                                <th>"Status"</th>
                                                <th>"From"</th>
                                            </tr>
                                        </thead>
                                        <tbody>
                                            {payments.clone().into_iter().map(|payment| {
                                                let tx_display = truncate_hex(&payment.tx_hash, 10, 8);
                                                let pstatus = payment_status(&payment);
                                                let pstatus_class = payment_status_class(&payment);
                                                let from_display = payment.from_address.as_ref()
                                                    .map(|a| truncate_hex(a, 8, 6))
                                                    .unwrap_or_else(|| "\u{2014}".to_string());

                                                view! {
                                                    <tr class="payment-row">
                                                        <td>
                                                            <div class="payment-tx-cell">
                                                                <code class="tx-hash">{tx_display}</code>
                                                                <button
                                                                    class="ps-btn-icon-xs"
                                                                    disabled=true
                                                                    title="Explorer link is not implemented yet"
                                                                >
                                                                    <IconExternalLink />
                                                                </button>
                                                            </div>
                                                        </td>
                                                        <td>
                                                            <span class="payment-amount">{payment.asset_symbol}</span>
                                                        </td>
                                                        <td>
                                                            <span class=pstatus_class>{pstatus}</span>
                                                        </td>
                                                        <td>
                                                            <code class="tx-hash">{from_display}</code>
                                                        </td>
                                                    </tr>
                                                }
                                            }).collect_view()}
                                        </tbody>
                                    </table>
                                </div>
                            }.into_any()
                        }}
                    </div>
                </div>

                // Payment Options Card (show available payment methods)
                {(!invoice.payment_options.is_empty()).then(|| {
                    let options = invoice.payment_options.clone();
                    view! {
                        <div class="ps-card">
                            <div class="ps-card-header">
                                <h3>"Payment Options"</h3>
                            </div>
                            <div class="ps-card-body">
                                {options.into_iter().map(|opt| {
                                    let addr_display = truncate_hex(&opt.payment_address, 10, 6);
                                    view! {
                                        <div class="detail-row">
                                            <span class="detail-label">
                                                {format!("{} ({})", opt.asset_symbol, chain_name(&opt.chain_id))}
                                            </span>
                                            <code class="detail-value">{addr_display}</code>
                                        </div>
                                    }
                                }).collect_view()}
                            </div>
                        </div>
                    }
                })}

                // Timeline Card
                <div class="ps-card">
                    <div class="ps-card-header">
                        <h3>"Activity"</h3>
                    </div>
                    <div class="ps-card-body">
                        <div class="timeline">
                            {is_paid.then(|| view! {
                                <div class=TimelineState::Done.row_class()>
                                    <div class="timeline-dot"></div>
                                    <div class="timeline-content">
                                        <span class="timeline-title">"Invoice paid in full"</span>
                                    </div>
                                </div>
                            })}
                            {(is_expired && !is_paid).then(|| view! {
                                <div class=TimelineState::Failed.row_class()>
                                    <div class="timeline-dot"></div>
                                    <div class="timeline-content">
                                        <span class="timeline-title">"Invoice expired"</span>
                                    </div>
                                </div>
                            })}
                            {payments.iter().map(|p| {
                                let timestamp = format_date(&p.detected_at.to_rfc3339());
                                let desc = format!("{} payment received", p.asset_symbol);
                                // Per payment, not per invoice: an invoice can
                                // hold a confirmed payment and a reorged one at
                                // once, and a single grey dot said neither.
                                let state = payment_state(p.reorged, p.confirmed_at.is_some());
                                view! {
                                    <div class=state.row_class()>
                                        <div class="timeline-dot"></div>
                                        <div class="timeline-content">
                                            <span class="timeline-title">{desc}</span>
                                            <span class="timeline-time">{timestamp}</span>
                                        </div>
                                    </div>
                                }
                            }).collect_view()}
                            <div class=TimelineState::Done.row_class()>
                                <div class="timeline-dot"></div>
                                <div class="timeline-content">
                                    <span class="timeline-title">"Invoice created"</span>
                                    <span class="timeline-time">{created_display}</span>
                                </div>
                            </div>
                        </div>
                    </div>
                </div>
            </div>

            // Sidebar
            <div class="invoice-detail-sidebar">
                <div class="ps-card">
                    <div class="ps-card-header">
                        <h3>"Customer"</h3>
                    </div>
                    <div class="ps-card-body">
                        <div class="customer-info">
                            <div class="customer-avatar">
                                {avatar_letter}
                            </div>
                            <div class="customer-details">
                                <span class="customer-email">
                                    {email_display}
                                </span>
                            </div>
                        </div>
                    </div>
                </div>

                <div class="ps-card">
                    <div class="ps-card-header">
                        <h3>"Status Details"</h3>
                    </div>
                    <div class="ps-card-body">
                        <div class="detail-row">
                            <span class="detail-label">"Paid"</span>
                            <span class="detail-value">{if is_paid { "Yes" } else { "No" }}</span>
                        </div>
                        <div class="detail-row">
                            <span class="detail-label">"Expired"</span>
                            <span class="detail-value">{if is_expired { "Yes" } else { "No" }}</span>
                        </div>
                    </div>
                </div>
            </div>
        </div>
    }
}

/// Clock icon for empty payments state.
#[component]
fn IconClock() -> impl IntoView {
    view! {
        <svg xmlns="http://www.w3.org/2000/svg" width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
            <circle cx="12" cy="12" r="10"></circle>
            <polyline points="12 6 12 12 16 14"></polyline>
        </svg>
    }
}

/// External link icon.
#[component]
fn IconExternalLink() -> impl IntoView {
    view! {
        <svg xmlns="http://www.w3.org/2000/svg" width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
            <path d="M18 13v6a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2V8a2 2 0 0 1 2-2h6"></path>
            <polyline points="15 3 21 3 21 9"></polyline>
            <line x1="10" y1="14" x2="21" y2="3"></line>
        </svg>
    }
}

/// Left arrow icon for the back link.
#[component]
fn IconArrowLeft() -> impl IntoView {
    view! {
        <svg xmlns="http://www.w3.org/2000/svg" width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
            <line x1="19" y1="12" x2="5" y2="12"></line>
            <polyline points="12 19 5 12 12 5"></polyline>
        </svg>
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::api::InvoiceStatus;
    use types::ChainId;

    fn sample_invoice() -> Invoice {
        Invoice {
            id: "inv-1".into(),
            store_id: "store-1".into(),
            store_name: None,
            currency: "USD".into(),
            status: InvoiceStatus::Paid,
            customer_email: None,
            amount: "30.000000000000000000".into(),
            amount_received: "30.000000000000000000".into(),
            created_at: "2024-01-01T00:00:00Z".parse().unwrap(),
            expires_at: "2024-01-02T00:00:00Z".parse().unwrap(),
            metadata: Some(serde_json::json!({"order_id": "ORD, 1", "buyer_email": "a@b.com"})),
            payment_options: vec![],
        }
    }

    fn sample_payment() -> Payment {
        Payment {
            id: "p1".into(),
            store_id: None,
            store_name: None,
            chain_id: ChainId::evm(1),
            invoice_id: "inv-1".into(),
            amount: "30".into(),
            asset_symbol: "ETH".into(),
            token_address: None,
            tx_hash: "0xabc".into(),
            block_number: Some(1),
            detected_at: "2024-01-01T00:05:00Z".parse().unwrap(),
            confirmed_at: Some("2024-01-01T00:10:00Z".parse().unwrap()),
            from_address: Some("0xdef".into()),
            reorged: false,
            decimals: 18,
        }
    }

    #[test]
    fn csv_escape_quotes_only_when_needed() {
        assert_eq!(csv_escape("plain"), "plain");
        assert_eq!(csv_escape("a,b"), "\"a,b\"");
        assert_eq!(csv_escape("a\"b"), "\"a\"\"b\"");
        assert_eq!(csv_escape("a\nb"), "\"a\nb\"");
    }

    #[test]
    fn invoice_csv_carries_the_raw_amount_and_escapes_metadata() {
        let csv = invoice_csv(&sample_invoice(), &[sample_payment()]);
        let mut lines = csv.lines();
        assert_eq!(
            lines.next().unwrap(),
            "id,store_id,status,currency,amount,amount_received,created_at,expires_at,order_id,customer_email"
        );
        // The order_id holds a comma, so it must come back quoted rather than
        // splitting the row into an extra column.
        assert_eq!(
            lines.next().unwrap(),
            "inv-1,store-1,Paid,USD,30.000000000000000000,30.000000000000000000,2024-01-01T00:00:00+00:00,2024-01-02T00:00:00+00:00,\"ORD, 1\",a@b.com"
        );
        assert!(csv.contains(
            "payment_tx_hash,asset_symbol,amount,status,from_address,detected_at,confirmed_at"
        ));
        assert!(csv.contains(
            "0xabc,ETH,30,confirmed,0xdef,2024-01-01T00:05:00+00:00,2024-01-01T00:10:00+00:00"
        ));
    }

    #[test]
    fn invoice_csv_omits_the_payments_section_when_there_are_none() {
        let csv = invoice_csv(&sample_invoice(), &[]);
        assert!(!csv.contains("payment_tx_hash"));
    }
}
