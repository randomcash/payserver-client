//! Shapes the server does not define, and presentation helpers over shared ones.
//!
//! Everything here is genuinely client-side. Anything the API actually speaks
//! belongs in `api-types` so both ends compile against one definition.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use types::InvoiceStatus;
use uuid::Uuid;

use super::Store;

/// How an invoice status is shown.
///
/// Labels and CSS classes are a rendering decision, so they stay here rather
/// than in the contract - the server has no opinion on what colour "expired" is.
pub trait InvoiceStatusExt {
    fn label(&self) -> &'static str;
    fn css_class(&self) -> &'static str;
}

impl InvoiceStatusExt for InvoiceStatus {
    fn label(&self) -> &'static str {
        match self {
            InvoiceStatus::Pending => "Pending",
            InvoiceStatus::Processing => "Processing",
            InvoiceStatus::PartiallyPaid => "Partially Paid",
            InvoiceStatus::Paid => "Paid",
            InvoiceStatus::Expired => "Expired",
            InvoiceStatus::Cancelled => "Cancelled",
            InvoiceStatus::Refunded => "Refunded",
            InvoiceStatus::LatePaid => "Late Paid",
        }
    }

    fn css_class(&self) -> &'static str {
        match self {
            InvoiceStatus::Pending => "badge badge-warning",
            InvoiceStatus::Processing => "badge badge-info",
            InvoiceStatus::PartiallyPaid => "badge badge-warning",
            InvoiceStatus::Paid => "badge badge-success",
            InvoiceStatus::Expired => "badge badge-error",
            InvoiceStatus::Cancelled => "badge badge-neutral",
            InvoiceStatus::Refunded => "badge badge-neutral",
            InvoiceStatus::LatePaid => "badge badge-info",
        }
    }
}

/// A role a user holds on a store.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StoreRole {
    pub id: String,
    pub store_id: Option<String>,
    pub role: String,
    pub permissions: Vec<String>,
}

/// A store together with the caller's role on it.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserStoreInfo {
    pub store: Store,
    pub role: StoreRole,
}

/// One page a plugin serves, flattened for navigation.
///
/// The server groups pages by plugin; navigation wants a flat list, and the
/// plugin id has to travel with each entry because it is half the URL.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PluginPage {
    pub plugin_id: String,
    pub path: String,
    pub label: String,
    pub icon: payserver_plugin_api::PageIcon,
}

/// `GET /api/plugins`, as the server sends it.
#[derive(Debug, Clone, Deserialize)]
pub struct PluginPagesResponse {
    pub plugins: Vec<PluginPagesInfo>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct PluginPagesInfo {
    pub id: String,
    pub pages: Vec<PluginPageInfo>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct PluginPageInfo {
    pub path: String,
    pub label: String,
    pub icon: payserver_plugin_api::PageIcon,
}

// =========================================================================
// API key permission scope
//
// Hand-mirrors the server's `permissions` field on API key requests and
// responses rather than extending the pinned `api_types` structs for them:
// `api-types` lives in payserver-commons, and landing a field there is the
// three-step dance (merge, bump the pinned rev, `cargo update`) described in
// this repo's `CLAUDE.md` - a cross-repo change this ticket cannot complete
// on its own. The wire shape only grows a field on top of the pinned types,
// so nothing about the existing `ApiKeyInfo`/`CreateApiKeyRequest` aliases
// breaks; these are additional types for the endpoints that need the extra
// field.
// =========================================================================

/// Unrestricted access: equivalent to the key owner's full role, including
/// installing a plugin, which runs arbitrary SQL migrations and arbitrary
/// wasm on the server. Kept apart from the store actions below - it also
/// covers `ethpay.server.*`/`ethpay.user.*` gates, none of which this
/// server enforces individually (every one is a bare
/// `role == Role::ServerAdmin` comparison), so a checkbox for one of those
/// specifically would promise control the server cannot back up.
pub const API_KEY_UNRESTRICTED_PERMISSION: &str = "unrestricted";

/// Store-scoped actions this server enforces individually, in SQL, at real
/// call sites (invoice creation, store settings, store membership) - see
/// `user_has_store_permission` and its callers in `server/src/api/`. Unlike
/// the server/user policies folded into "unrestricted" above, checking one
/// of these and leaving the rest unchecked genuinely grants only that one.
///
/// Hand-mirrored from `auth::Policies` (payserver-commons) plus two more
/// (`canviewstoreusers`/`canmodifystoreusers`) that exist only as literal
/// strings on the server side today, not yet promoted to that enum.
///
/// Each entry a key requests here is granted unscoped - every store the
/// owner can reach - which is the server's default and the only shape this
/// form produces. The server also accepts a `policy:storeId` suffix to
/// narrow a grant to one store; there is no control for that yet, so a key
/// narrower than "every store" has to be built directly against the API.
pub const API_KEY_GRANTABLE_STORE_ACTIONS: &[(&str, &str)] = &[
    ("ethpay.store.cancreateinvoice", "Create invoices"),
    ("ethpay.store.canviewstoresettings", "View store settings"),
    (
        "ethpay.store.canmodifystoresettings",
        "Modify store settings",
    ),
    ("ethpay.store.canviewstoreusers", "View store members"),
    ("ethpay.store.canmodifystoreusers", "Manage store members"),
];

/// Whether a stored permission scope grants full, unrestricted access - the
/// same test the server applies when deciding whether to downgrade a
/// request's effective role. `None` (inherits the owner's role in full)
/// counts, same as an explicit `["unrestricted"]` entry.
pub fn api_key_is_unrestricted(permissions: &Option<Vec<String>>) -> bool {
    match permissions {
        None => true,
        Some(perms) => perms.iter().any(|p| p == API_KEY_UNRESTRICTED_PERMISSION),
    }
}

/// Human-readable summary of an API key's scope, for the key list - the
/// whole point being that "testnet e2e key" should never again read as
/// harmless when it is not.
pub fn describe_api_key_permissions(permissions: &Option<Vec<String>>) -> String {
    if api_key_is_unrestricted(permissions) {
        return match permissions {
            None => "Full access (inherits your role)".to_string(),
            Some(_) => "Full access (unrestricted)".to_string(),
        };
    }
    let granted = permissions.as_deref().unwrap_or(&[]);
    if granted.is_empty() {
        return "No permissions granted".to_string();
    }
    granted
        .iter()
        .map(|entry| {
            // A `policy:storeId` entry describes the same action as its
            // unscoped form for this summary - the list view has no room to
            // also name the store.
            let policy = entry.split(':').next().unwrap_or(entry);
            API_KEY_GRANTABLE_STORE_ACTIONS
                .iter()
                .find(|(p, _)| *p == policy)
                .map_or(policy, |(_, label)| label)
        })
        .collect::<Vec<_>>()
        .join(", ")
}

/// `CreateApiKeyRequest` (api-types) plus the permission scope chosen at
/// creation.
#[derive(Debug, Clone, Serialize)]
pub struct CreateApiKeyRequestWithPermissions {
    pub name: String,
    pub expires_at: Option<DateTime<Utc>>,
    pub permissions: Vec<String>,
}

/// `ApiKeyInfo` (api-types) plus the permission scope.
#[derive(Debug, Clone, Deserialize)]
pub struct ApiKeyInfoWithPermissions {
    pub id: Uuid,
    pub name: String,
    pub key_prefix: String,
    pub is_active: bool,
    pub created_at: DateTime<Utc>,
    pub last_used_at: Option<DateTime<Utc>>,
    pub expires_at: Option<DateTime<Utc>>,
    pub rate_limit_rpm: Option<i32>,
    pub deprecated_at: Option<DateTime<Utc>>,
    pub deprecation_expires_at: Option<DateTime<Utc>>,
    /// `None` means the key inherits its owner's role in full.
    pub permissions: Option<Vec<String>>,
}

/// `GET /api/users/api-keys`, with permission scope.
#[derive(Debug, Clone, Deserialize)]
pub struct ApiKeyListResponseWithPermissions {
    pub keys: Vec<ApiKeyInfoWithPermissions>,
}

/// `POST /api/users/api-keys` response, with permission scope.
#[derive(Debug, Clone, Deserialize)]
pub struct CreateApiKeyResponseWithPermissions {
    pub id: Uuid,
    pub name: String,
    pub key_prefix: String,
    pub is_active: bool,
    pub created_at: DateTime<Utc>,
    pub expires_at: Option<DateTime<Utc>>,
    /// The plaintext API key. Store this securely — it cannot be retrieved again.
    pub key: String,
    pub permissions: Vec<String>,
}

/// `POST /api/users/api-keys/{id}/rotate` response, with permission scope.
#[derive(Debug, Clone, Deserialize)]
pub struct RotateApiKeyResponseWithPermissions {
    pub id: Uuid,
    pub name: String,
    pub key_prefix: String,
    pub created_at: DateTime<Utc>,
    pub key: String,
    pub old_key_deprecated_at: DateTime<Utc>,
    pub old_key_grace_expires_at: DateTime<Utc>,
    pub permissions: Option<Vec<String>>,
}

/// Body for `PATCH /api/users/api-keys/{id}/permissions`.
#[derive(Debug, Clone, Serialize)]
pub struct UpdateApiKeyPermissionsRequest {
    /// `None` clears the key back to "inherit the owner's role in full".
    pub permissions: Option<Vec<String>>,
}
