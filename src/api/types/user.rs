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

/// Body for `POST /users/me/email`.
///
/// Mirrors `RequestEmailChangePayload` (server/src/api/users.rs) rather than
/// coming from `api-types` like most request bodies here - that type's doc
/// comment explains why: `api-types` lives in `payserver-commons`, and this
/// ticket cannot complete the merge-then-bump-rev cycle landing something
/// there requires on its own. Two primitive fields, so the drift risk of a
/// hand-mirrored copy is small.
#[derive(Debug, Clone, Serialize)]
pub struct RequestEmailChangeRequest {
    pub new_email: String,
}

/// Body for `POST /users/me/email/confirm`. See `RequestEmailChangeRequest`.
#[derive(Debug, Clone, Serialize)]
pub struct ConfirmEmailChangeRequest {
    pub token: String,
}
