//! User-related API types.

use serde::{Deserialize, Serialize};

/// User role.
///
/// Mirrors `Role` from the auth crate.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum UserRole {
    ServerAdmin,
    #[default]
    User,
}

impl UserRole {
    pub fn label(&self) -> &'static str {
        match self {
            UserRole::ServerAdmin => "Server Admin",
            UserRole::User => "User",
        }
    }

    pub fn is_admin(&self) -> bool {
        matches!(self, UserRole::ServerAdmin)
    }
}

/// Authenticated user info from `/auth/me`.
///
/// Mirrors `UserInfo` from the auth crate.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserInfo {
    pub id: String,
    pub email: Option<String>,
    pub primary_wallet_address: Option<String>,
    pub created_at: String,
    pub last_login_at: Option<String>,
    pub role: UserRole,
}

/// A wallet login credential belonging to this account.
///
/// Not the same thing as `Wallet` (`WalletResponse` from `api-types`), which
/// is an xpub payout wallet the account receives crypto into. This is the
/// Ethereum address that can sign in as this account; `is_primary` marks the
/// one wallet login resolves the account by.
///
/// Hand-mirrors the server's `WalletCredentialResponse` rather than pulling
/// it from `api-types`: that DTO can't be added there in this change -
/// `api-types` is pinned by `rev` in a sibling repository, and moving the pin
/// is its own three-step change (merge in commons, bump the rev, `cargo
/// update`).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WalletCredential {
    pub id: String,
    pub address: String,
    pub name: String,
    pub is_primary: bool,
    pub created_at: String,
    pub last_used_at: Option<String>,
}

/// The message to sign to prove current ownership of a wallet credential,
/// returned by `POST /api/users/wallets/{id}/reauth-challenge`.
///
/// Hand-mirrors the server's `WalletReauthChallengeResponse` for the same
/// reason as `WalletCredential` above.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WalletReauthChallenge {
    pub message: String,
    pub expires_in_secs: i64,
}

/// Body for `PATCH /api/users/wallets/{id}/primary`.
///
/// Hand-mirrors the server's `PromoteWalletCredentialRequest`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PromoteWalletCredentialRequest {
    pub signature: String,
}
