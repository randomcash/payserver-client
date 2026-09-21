//! Shared helpers and icons for the invoice pages.

use leptos::prelude::*;

use crate::api::{Invoice, Payment};

pub(super) use crate::util::chain_name;

/// Helper to determine payment status from confirmed_at.
pub(super) fn payment_status(payment: &Payment) -> &'static str {
    if payment.reorged {
        "reorged"
    } else if payment.confirmed_at.is_some() {
        "confirmed"
    } else {
        "confirming"
    }
}

/// CSS class for payment status.
pub(super) fn payment_status_class(payment: &Payment) -> &'static str {
    if payment.reorged {
        "badge badge-error"
    } else if payment.confirmed_at.is_some() {
        "badge badge-success"
    } else {
        "badge badge-warning"
    }
}

/// Helper to extract a field from invoice metadata.
pub(super) fn get_metadata_field(invoice: &Invoice, field: &str) -> Option<String> {
    invoice
        .metadata
        .as_ref()
        .and_then(|m: &serde_json::Value| m.get(field))
        .and_then(|v: &serde_json::Value| v.as_str())
        .map(|s: &str| s.to_string())
}

/// Format ISO date string for display.
pub(super) fn format_date(iso: &str) -> String {
    // Simple formatting - in production would use chrono
    if iso.len() >= 10 {
        let date_part = &iso[..10];
        // Parse YYYY-MM-DD and format as "Jan DD, YYYY"
        let parts: Vec<&str> = date_part.split('-').collect();
        if parts.len() == 3 {
            let month = match parts[1] {
                "01" => "Jan",
                "02" => "Feb",
                "03" => "Mar",
                "04" => "Apr",
                "05" => "May",
                "06" => "Jun",
                "07" => "Jul",
                "08" => "Aug",
                "09" => "Sep",
                "10" => "Oct",
                "11" => "Nov",
                "12" => "Dec",
                _ => parts[1],
            };
            return format!("{} {}, {}", month, parts[2], parts[0]);
        }
    }
    iso.to_string()
}

/// Format an amount with its currency (e.g., "$100.00 USD", "0.5 ETH").
///
/// `amount` arrives as the raw `NUMERIC(38,18)` string the server stores it
/// as, so "30" comes back as "30.000000000000000000". Fiat currencies get
/// rounded to their usual two decimals; anything else just loses the
/// trailing zeros the database carries and nobody reads.
pub(super) fn format_amount(amount: &str, currency: &str) -> String {
    match currency {
        "USD" => format!("${} {}", round_decimal(amount, 2), currency),
        "EUR" => format!("\u{20ac}{} {}", round_decimal(amount, 2), currency),
        "GBP" => format!("\u{00a3}{} {}", round_decimal(amount, 2), currency),
        _ => format!("{} {}", trim_trailing_zeros(amount), currency),
    }
}

/// Drop trailing zeros from a decimal string's fractional part.
///
/// "1.5000" -> "1.5", "30.000000000000000000" -> "30".
fn trim_trailing_zeros(amount: &str) -> String {
    match amount.split_once('.') {
        Some((int_part, frac_part)) => {
            let trimmed = frac_part.trim_end_matches('0');
            if trimmed.is_empty() {
                int_part.to_string()
            } else {
                format!("{int_part}.{trimmed}")
            }
        }
        None => amount.to_string(),
    }
}

/// Round a decimal string to a fixed number of places.
///
/// Half-up on the first dropped digit is exact here: everything past that
/// digit can never carry the remainder across the halfway point, so it alone
/// decides the direction. Falls back to the original string on anything that
/// isn't the plain decimal the server always sends, rather than panic on it.
fn round_decimal(amount: &str, places: usize) -> String {
    let (int_part, frac_part) = amount.split_once('.').unwrap_or((amount, ""));
    let Ok(mut int_val) = int_part.parse::<u128>() else {
        return amount.to_string();
    };
    if !frac_part.bytes().all(|b| b.is_ascii_digit()) {
        return amount.to_string();
    }

    let frac_digits: Vec<u8> = frac_part.bytes().map(|b| b - b'0').collect();
    let mut kept: Vec<u8> = (0..places)
        .map(|i| *frac_digits.get(i).unwrap_or(&0))
        .collect();
    let round_up = frac_digits.get(places).copied().unwrap_or(0) >= 5;

    if round_up {
        let mut i = places;
        loop {
            if i == 0 {
                int_val += 1;
                break;
            }
            i -= 1;
            if kept[i] == 9 {
                kept[i] = 0;
            } else {
                kept[i] += 1;
                break;
            }
        }
    }

    if places == 0 {
        int_val.to_string()
    } else {
        let frac_str: String = kept.iter().map(|d| (d + b'0') as char).collect();
        format!("{int_val}.{frac_str}")
    }
}

/// Count confirmed (non-reorged) payments.
pub(super) fn confirmed_payment_count(payments: &[Payment]) -> usize {
    payments
        .iter()
        .filter(|p| p.confirmed_at.is_some() && !p.reorged)
        .count()
}

/// Truncate a hex string for display (e.g., "0x1234abcd...5678ef01").
pub(super) fn truncate_hex(s: &str, prefix_len: usize, suffix_len: usize) -> String {
    if s.len() > prefix_len + suffix_len + 3 {
        format!("{}...{}", &s[..prefix_len], &s[s.len() - suffix_len..])
    } else {
        s.to_string()
    }
}

/// Export/download icon (shared by the list and detail pages).
#[component]
pub(super) fn IconExport() -> impl IntoView {
    view! {
        <svg xmlns="http://www.w3.org/2000/svg" width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
            <path d="M21 15v4a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2v-4"></path>
            <polyline points="17 8 12 3 7 8"></polyline>
            <line x1="12" y1="3" x2="12" y2="15"></line>
        </svg>
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::api::InvoiceStatus;
    use types::ChainId;

    #[test]
    fn test_format_date_iso() {
        assert_eq!(format_date("2024-01-15T10:30:00Z"), "Jan 15, 2024");
        assert_eq!(format_date("2024-12-25T00:00:00Z"), "Dec 25, 2024");
    }

    #[test]
    fn test_format_date_all_months() {
        let months = [
            ("01", "Jan"),
            ("02", "Feb"),
            ("03", "Mar"),
            ("04", "Apr"),
            ("05", "May"),
            ("06", "Jun"),
            ("07", "Jul"),
            ("08", "Aug"),
            ("09", "Sep"),
            ("10", "Oct"),
            ("11", "Nov"),
            ("12", "Dec"),
        ];
        for (num, name) in months {
            let input = format!("2024-{}-01T00:00:00Z", num);
            assert!(
                format_date(&input).starts_with(name),
                "Failed for month {}",
                num
            );
        }
    }

    #[test]
    fn test_format_date_short_string() {
        assert_eq!(format_date("short"), "short");
        assert_eq!(format_date(""), "");
    }

    #[test]
    fn test_chain_name() {
        assert_eq!(chain_name(&ChainId::evm(1)), "Ethereum");
        assert_eq!(chain_name(&ChainId::evm(137)), "Polygon");
        assert_eq!(chain_name(&ChainId::evm(42161)), "Arbitrum");
        assert_eq!(chain_name(&ChainId::evm(10)), "Optimism");
        assert_eq!(chain_name(&ChainId::evm(8453)), "Base");
        assert_eq!(chain_name(&ChainId::evm(56)), "BSC");
        assert_eq!(chain_name(&ChainId::evm(43114)), "Avalanche");
        assert_eq!(chain_name(&ChainId::evm(11155111)), "Sepolia");
        // An unnamed chain renders as its own identifier rather than "Unknown":
        // ugly but true, and it lets a reader work out which chain it is.
        assert_eq!(chain_name(&ChainId::evm(99999)), "eip155:99999");
    }

    #[test]
    fn test_payment_status_confirmed() {
        let p = Payment {
            id: "p1".into(),
            store_id: None,
            store_name: None,
            chain_id: ChainId::evm(1),
            invoice_id: "inv-1".into(),
            amount: "100".into(),
            asset_symbol: "ETH".into(),
            token_address: None,
            tx_hash: "0xabc".into(),
            block_number: Some(1),
            detected_at: "2024-01-01T00:00:00Z".parse().unwrap(),
            confirmed_at: Some("2024-01-01T00:05:00Z".parse().unwrap()),
            from_address: None,
            reorged: false,
            decimals: 18,
        };
        assert_eq!(payment_status(&p), "confirmed");
        assert_eq!(payment_status_class(&p), "badge badge-success");
    }

    #[test]
    fn test_payment_status_confirming() {
        let p = Payment {
            id: "p2".into(),
            store_id: None,
            store_name: None,
            chain_id: ChainId::evm(1),
            invoice_id: "inv-1".into(),
            amount: "100".into(),
            asset_symbol: "ETH".into(),
            token_address: None,
            tx_hash: "0xdef".into(),
            block_number: None,
            detected_at: "2024-01-01T00:00:00Z".parse().unwrap(),
            confirmed_at: None,
            from_address: None,
            reorged: false,
            decimals: 18,
        };
        assert_eq!(payment_status(&p), "confirming");
        assert_eq!(payment_status_class(&p), "badge badge-warning");
    }

    #[test]
    fn test_payment_status_reorged() {
        let p = Payment {
            id: "p3".into(),
            store_id: None,
            store_name: None,
            chain_id: ChainId::evm(1),
            invoice_id: "inv-1".into(),
            amount: "100".into(),
            asset_symbol: "ETH".into(),
            token_address: None,
            tx_hash: "0xghi".into(),
            block_number: Some(1),
            detected_at: "2024-01-01T00:00:00Z".parse().unwrap(),
            confirmed_at: Some("2024-01-01T00:05:00Z".parse().unwrap()),
            from_address: None,
            reorged: true,
            decimals: 18,
        };
        assert_eq!(payment_status(&p), "reorged");
        assert_eq!(payment_status_class(&p), "badge badge-error");
    }

    #[test]
    fn test_get_metadata_field() {
        let invoice = Invoice {
            id: "inv-1".into(),
            store_id: "store-001".to_string(),
            store_name: None,
            currency: "USD".into(),
            status: InvoiceStatus::Pending,
            customer_email: None,
            amount: "100".into(),
            amount_received: "0".into(),
            created_at: "2024-01-01T00:00:00Z".parse().unwrap(),
            expires_at: "2024-01-02T00:00:00Z".parse().unwrap(),
            metadata: Some(serde_json::json!({"order_id": "ORD-1", "customer_email": "a@b.com"})),
            payment_options: vec![],
        };
        assert_eq!(
            get_metadata_field(&invoice, "order_id"),
            Some("ORD-1".to_string())
        );
        assert_eq!(
            get_metadata_field(&invoice, "customer_email"),
            Some("a@b.com".to_string())
        );
        assert_eq!(get_metadata_field(&invoice, "nonexistent"), None);
    }

    #[test]
    fn test_get_metadata_field_no_metadata() {
        let invoice = Invoice {
            id: "inv-2".into(),
            store_id: "store-001".to_string(),
            store_name: None,
            currency: "USD".into(),
            status: InvoiceStatus::Pending,
            customer_email: None,
            amount: "50".into(),
            amount_received: "0".into(),
            created_at: "2024-01-01T00:00:00Z".parse().unwrap(),
            expires_at: "2024-01-02T00:00:00Z".parse().unwrap(),
            metadata: None,
            payment_options: vec![],
        };
        assert_eq!(get_metadata_field(&invoice, "order_id"), None);
    }

    #[test]
    fn test_format_amount() {
        assert_eq!(format_amount("100.00", "USD"), "$100.00 USD");
        assert_eq!(format_amount("50.00", "EUR"), "\u{20ac}50.00 EUR");
        assert_eq!(format_amount("25.00", "GBP"), "\u{00a3}25.00 GBP");
        assert_eq!(format_amount("1.5", "ETH"), "1.5 ETH");
        assert_eq!(format_amount("0.001", "BTC"), "0.001 BTC");
    }

    #[test]
    fn test_format_amount_raw_numeric_precision() {
        // What the server actually sends: NUMERIC(38,18) serialized straight
        // to a string. The merchant's most-looked-at page must not show it.
        assert_eq!(format_amount("30.000000000000000000", "USD"), "$30.00 USD");
        assert_eq!(format_amount("0.000000000000000000", "USD"), "$0.00 USD");
        assert_eq!(format_amount("1.500000000000000000", "ETH"), "1.5 ETH");
    }

    #[test]
    fn test_round_decimal_rounds_half_up() {
        assert_eq!(round_decimal("29.995000000000000000", 2), "30.00");
        assert_eq!(round_decimal("29.994999999999999999", 2), "29.99");
        assert_eq!(round_decimal("9.995", 2), "10.00");
        assert_eq!(round_decimal("100", 2), "100.00");
    }

    #[test]
    fn test_confirmed_payment_count() {
        let payments = vec![
            Payment {
                id: "p1".into(),
                store_id: None,
                store_name: None,
                chain_id: ChainId::evm(1),
                invoice_id: "inv-1".into(),
                amount: "100".into(),
                asset_symbol: "ETH".into(),
                token_address: None,
                tx_hash: "0xabc".into(),
                block_number: Some(1),
                detected_at: "2024-01-01T00:00:00Z".parse().unwrap(),
                confirmed_at: Some("2024-01-01T00:05:00Z".parse().unwrap()),
                from_address: None,
                reorged: false,
                decimals: 18,
            },
            Payment {
                id: "p2".into(),
                store_id: None,
                store_name: None,
                chain_id: ChainId::evm(1),
                invoice_id: "inv-1".into(),
                amount: "50".into(),
                asset_symbol: "ETH".into(),
                token_address: None,
                tx_hash: "0xdef".into(),
                block_number: None,
                detected_at: "2024-01-01T00:00:00Z".parse().unwrap(),
                confirmed_at: None,
                from_address: None,
                reorged: false,
                decimals: 18,
            },
            Payment {
                id: "p3".into(),
                store_id: None,
                store_name: None,
                chain_id: ChainId::evm(1),
                invoice_id: "inv-1".into(),
                amount: "75".into(),
                asset_symbol: "ETH".into(),
                token_address: None,
                tx_hash: "0xghi".into(),
                block_number: Some(2),
                detected_at: "2024-01-01T00:00:00Z".parse().unwrap(),
                confirmed_at: Some("2024-01-01T00:10:00Z".parse().unwrap()),
                from_address: None,
                reorged: true,
                decimals: 18,
            },
        ];
        // Only p1 is confirmed and not reorged
        assert_eq!(confirmed_payment_count(&payments), 1);
        assert_eq!(confirmed_payment_count(&[]), 0);
    }

    #[test]
    fn test_truncate_hex() {
        // Long hex gets truncated
        assert_eq!(
            truncate_hex("0x1234567890abcdef1234567890abcdef", 10, 8),
            "0x12345678...90abcdef"
        );
        // Short hex stays as-is
        assert_eq!(truncate_hex("0xabc", 10, 8), "0xabc");
        // Exactly at boundary
        assert_eq!(
            truncate_hex("0x1234567890abcdef12", 10, 8),
            "0x1234567890abcdef12"
        );
    }
}
