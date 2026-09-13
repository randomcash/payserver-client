//! Auth/user, dashboard, API key, and admin API methods.

use super::{ApiClient, ApiError};
use crate::api::{
    ApiKeyListResponse, ConfirmEmailChangeRequest, CreateApiKeyRequest,
    CreateApiKeyResponsePayload, DashboardAnalytics, DashboardStats, RequestEmailChangeRequest,
    RotateApiKeyResponse, ServerSettingsResponse, UpdateServerSettingsRequest,
    UpdateUserRoleRequest, UserInfo, UserListResponse,
};

impl ApiClient {
    // =========================================================================
    // Auth / User
    // =========================================================================

    /// Get the current authenticated user's info.
    pub async fn get_me(&self) -> Result<UserInfo, ApiError> {
        self.get("/api/auth/me").await
    }

    /// Log out the current session (server-side invalidation).
    /// Delete the authenticated account.
    ///
    /// `confirm` must be the account's email, or its id where it has none. The
    /// server refuses while the account's stores hold any payment, payout or
    /// refund - deleting would cascade through invoices into payments and erase
    /// a merchant's financial history - and answers 409 naming what blocked it.
    ///
    /// The confirmation is a query parameter rather than a body so that no DTO
    /// has to be defined twice, once here and once server-side.
    pub async fn delete_account(&self, confirm: &str) -> Result<(), ApiError> {
        self.delete(&format!(
            "/api/users/me?confirm={}",
            js_sys::encode_uri_component(confirm)
        ))
        .await
    }

    /// Start changing (or setting) the account's email address (RCS-263).
    ///
    /// Requires re-authentication: `self` must carry the session token from a
    /// **freshly completed** passkey or wallet login, not the caller's normal
    /// session - the server rejects a session it does not consider recent
    /// enough. See `ReauthGate` in `pages/settings/account.rs` for how that
    /// fresh session is obtained without disturbing the user's ambient login.
    ///
    /// Sends a verification code to `new_email`; call `confirm_email_change`
    /// with that code to complete the change. Fails (503) if the server has
    /// no SMTP configured, rather than queuing a change nobody can confirm.
    pub async fn request_email_change(&self, new_email: &str) -> Result<(), ApiError> {
        self.post_ignoring_response(
            "/api/users/me/email",
            &RequestEmailChangeRequest {
                new_email: new_email.to_string(),
            },
        )
        .await
    }

    /// Confirm a pending email change with the code emailed to the new
    /// address. Unauthenticated by design - the code itself, delivered only
    /// to that address, is the proof.
    pub async fn confirm_email_change(&self, token: &str) -> Result<(), ApiError> {
        self.post_ignoring_response(
            "/api/users/me/email/confirm",
            &ConfirmEmailChangeRequest {
                token: token.to_string(),
            },
        )
        .await
    }

    /// Remove the account's email (RCS-263). Requires a freshly completed
    /// login, same as `request_email_change`. Refused (409) if the account
    /// has no wallet - that would leave the account id as the only way back
    /// into recovery.
    pub async fn remove_email(&self) -> Result<(), ApiError> {
        self.delete("/api/users/me/email").await
    }

    pub async fn logout(&self) -> Result<(), ApiError> {
        let request = self
            .build_request("POST", "/api/auth/logout")
            .build()
            .map_err(|e| ApiError::Network(e.to_string()))?;

        let response = request
            .send()
            .await
            .map_err(|e| ApiError::Network(e.to_string()))?;

        if response.status() == 401 {
            return Err(ApiError::Unauthorized);
        }

        if !response.ok() {
            let message = response.text().await.unwrap_or_default();
            return Err(ApiError::Http {
                status: response.status(),
                message,
            });
        }

        Ok(())
    }

    // =========================================================================
    // Dashboard
    // =========================================================================

    /// Get dashboard statistics.
    pub async fn get_dashboard_stats(&self) -> Result<DashboardStats, ApiError> {
        self.get("/api/dashboard/stats").await
    }

    /// Get per-day, per-asset payment volume for the dashboard charts.
    ///
    /// `days` is the window size; the server rejects anything outside 1..=90
    /// rather than aggregating unbounded history.
    pub async fn get_dashboard_analytics(&self, days: u32) -> Result<DashboardAnalytics, ApiError> {
        self.get(&format!("/api/dashboard/analytics?days={days}"))
            .await
    }

    // =========================================================================
    // API Keys
    // =========================================================================

    /// List API keys for the authenticated user.
    pub async fn list_api_keys(&self) -> Result<ApiKeyListResponse, ApiError> {
        self.get("/api/users/api-keys").await
    }

    /// Create a new API key.
    pub async fn create_api_key(
        &self,
        request: &CreateApiKeyRequest,
    ) -> Result<CreateApiKeyResponsePayload, ApiError> {
        self.post("/api/users/api-keys", request).await
    }

    /// Revoke an API key.
    pub async fn revoke_api_key(&self, id: &str) -> Result<(), ApiError> {
        self.delete(&format!("/api/users/api-keys/{}", id)).await
    }

    /// Rotate an API key (deprecates old, creates new).
    pub async fn rotate_api_key(&self, id: &str) -> Result<RotateApiKeyResponse, ApiError> {
        self.post_empty(&format!("/api/users/api-keys/{}/rotate", id))
            .await
    }

    // =========================================================================
    // Admin
    // =========================================================================

    /// List all users (admin only).
    pub async fn list_users(&self, offset: i64, limit: i64) -> Result<UserListResponse, ApiError> {
        self.get(&format!(
            "/api/admin/users?offset={}&limit={}",
            offset, limit
        ))
        .await
    }

    /// Update a user's role (admin only).
    pub async fn update_user_role(
        &self,
        user_id: &str,
        request: &UpdateUserRoleRequest,
    ) -> Result<(), ApiError> {
        self.patch_empty(&format!("/api/admin/users/{}/role", user_id), request)
            .await
    }

    /// Lock a user account (admin only).
    pub async fn lock_user(&self, user_id: &str) -> Result<(), ApiError> {
        self.post_empty_body(&format!("/api/admin/users/{}/lock", user_id))
            .await
    }

    /// Unlock a user account (admin only).
    pub async fn unlock_user(&self, user_id: &str) -> Result<(), ApiError> {
        self.post_empty_body(&format!("/api/admin/users/{}/unlock", user_id))
            .await
    }

    /// Get server settings (admin only).
    pub async fn get_server_settings(&self) -> Result<ServerSettingsResponse, ApiError> {
        self.get("/api/admin/settings").await
    }

    /// Update server settings (admin only).
    pub async fn update_server_settings(
        &self,
        request: &UpdateServerSettingsRequest,
    ) -> Result<(), ApiError> {
        self.put_empty("/api/admin/settings", request).await
    }
}
