//! Invoice API methods.

use super::{ApiClient, ApiError};
use crate::api::{
    CheckoutResponse, CreateInvoiceRequest, Invoice, InvoiceListResponse, InvoiceStatusResponse,
    Payment, TxHashLookupResponse,
};

/// Percent-encodes a query parameter value the way `js_sys::encode_uri_component`
/// does, in plain Rust.
///
/// `list_invoices` is the one filtered list call a UI component (the wallet
/// rotation tab's outstanding-invoice count) needs to drive under `cargo
/// test`, and `js_sys::encode_uri_component` panics off a wasm host before
/// the request ever reaches the `get`/test-transport seam in `mod.rs`. This
/// keeps the request this function builds identical while making it possible
/// to run outside a browser.
fn encode_query_param(value: &str) -> String {
    let mut out = String::with_capacity(value.len());
    for byte in value.bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'!' | b'~' | b'*'
            | b'\'' | b'(' | b')' => out.push(byte as char),
            _ => out.push_str(&format!("%{:02X}", byte)),
        }
    }
    out
}

impl ApiClient {
    /// List invoices with filters and pagination.
    ///
    /// `store_id` of `None` means "All Stores": the server then returns
    /// invoices across every store, which it only allows for server admins —
    /// any other caller gets `400 Bad Request`.
    ///
    /// `search` is a free-text term over id, currency, amount and metadata,
    /// applied in SQL so `total` counts the same rows the page shows.
    pub async fn list_invoices(
        &self,
        store_id: Option<&str>,
        status: Option<&str>,
        currency: Option<&str>,
        search: Option<&str>,
        limit: Option<i64>,
        offset: Option<i64>,
    ) -> Result<InvoiceListResponse, ApiError> {
        let mut params = Vec::new();
        if let Some(sid) = store_id {
            params.push(format!("store_id={}", encode_query_param(sid)));
        }
        if let Some(s) = status {
            params.push(format!("status={}", encode_query_param(s)));
        }
        if let Some(c) = currency {
            params.push(format!("currency={}", encode_query_param(c)));
        }
        if let Some(q) = search {
            params.push(format!("search={}", encode_query_param(q)));
        }
        if let Some(l) = limit {
            params.push(format!("limit={}", l));
        }
        if let Some(o) = offset {
            params.push(format!("offset={}", o));
        }
        self.get(&format!("/api/invoices?{}", params.join("&")))
            .await
    }

    /// Get an invoice by ID.
    pub async fn get_invoice(&self, id: &str) -> Result<Invoice, ApiError> {
        self.get(&format!("/api/invoices/{}", id)).await
    }

    /// Get public checkout data for an invoice (no auth required).
    pub async fn get_checkout(&self, invoice_id: &str) -> Result<CheckoutResponse, ApiError> {
        self.get(&format!(
            "/api/checkout/{}",
            js_sys::encode_uri_component(invoice_id)
        ))
        .await
    }

    /// Create a new invoice.
    pub async fn create_invoice(
        &self,
        request: &CreateInvoiceRequest,
    ) -> Result<Invoice, ApiError> {
        self.post("/api/invoices", request).await
    }

    /// Get payments for an invoice.
    pub async fn get_invoice_payments(&self, invoice_id: &str) -> Result<Vec<Payment>, ApiError> {
        self.get(&format!("/api/invoices/{}/payments", invoice_id))
            .await
    }

    /// Get invoice status (includes payment options and payments).
    pub async fn get_invoice_status(
        &self,
        invoice_id: &str,
    ) -> Result<InvoiceStatusResponse, ApiError> {
        self.get(&format!("/api/invoices/{}/status", invoice_id))
            .await
    }

    /// Export invoices as CSV text.
    ///
    /// `store_id` of `None` exports across all stores (admins only) — see
    /// [`Self::list_invoices`].
    ///
    /// `search` is passed for the same reason `status` is: the export shares
    /// the list's filter builders server-side, and an export that ignores the
    /// search box hands the merchant rows they cannot see.
    pub async fn export_invoices_csv(
        &self,
        store_id: Option<&str>,
        status: Option<&str>,
        search: Option<&str>,
    ) -> Result<String, ApiError> {
        let mut params = Vec::new();
        if let Some(sid) = store_id {
            params.push(format!("store_id={}", js_sys::encode_uri_component(sid)));
        }
        if let Some(s) = status {
            params.push(format!("status={}", js_sys::encode_uri_component(s)));
        }
        if let Some(q) = search {
            params.push(format!("search={}", js_sys::encode_uri_component(q)));
        }
        self.get_text(&format!("/api/invoices/export.csv?{}", params.join("&")))
            .await
    }

    /// Look up an invoice by transaction hash.
    pub async fn lookup_invoice_by_tx(
        &self,
        chain_id: u64,
        tx_hash: &str,
    ) -> Result<TxHashLookupResponse, ApiError> {
        self.get(&format!("/api/invoices/by-tx/{}/{}", chain_id, tx_hash))
            .await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use super::super::{RequestSpec, TestTransport};
    use std::sync::{Arc, Mutex};

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

    #[test]
    fn encode_query_param_leaves_unreserved_characters_alone() {
        assert_eq!(encode_query_param("abcXYZ019-_.!~*'()"), "abcXYZ019-_.!~*'()");
    }

    #[test]
    fn encode_query_param_percent_encodes_everything_else() {
        assert_eq!(encode_query_param("pending invoice"), "pending%20invoice");
        assert_eq!(encode_query_param("a&b=c"), "a%26b%3Dc");
    }

    #[test]
    fn list_invoices_builds_the_filtered_path_without_a_wasm_host() {
        let recorded = Arc::new(Mutex::new(None));
        let recorded_clone = recorded.clone();
        let transport: TestTransport = Arc::new(move |spec| {
            *recorded_clone.lock().unwrap() = Some(spec);
            Ok(serde_json::json!({ "total": 0, "invoices": [] }))
        });
        let client = ApiClient::with_test_transport("", transport);

        let result = block_on(client.list_invoices(
            Some("store-1"),
            Some("pending"),
            None,
            None,
            Some(1),
            Some(0),
        ));

        assert!(result.is_ok());
        assert_eq!(
            recorded.lock().unwrap().clone().unwrap(),
            RequestSpec {
                method: "GET",
                path: "/api/invoices?store_id=store-1&status=pending&limit=1&offset=0"
                    .to_string(),
                body: None,
            }
        );
    }
}
