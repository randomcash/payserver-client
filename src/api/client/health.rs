//! Health API methods.

use super::{ApiClient, ApiError};
use crate::api::{ChainsHealthResponse, EmailStatus};

impl ApiClient {
    /// Fetch per-chain monitor health.
    ///
    /// Answers every caller. An admin additionally gets block heights, the
    /// watched-address count and the reason behind a failure; everyone else
    /// gets the chain's identity and whether it is up, which is the part a
    /// merchant needs.
    ///
    /// The server answers `503` when the monitor has published nothing to
    /// Redis, so an error here is a real "we do not know" and must not be
    /// rendered as healthy — a green panel over dead monitors has shipped
    /// before.
    ///
    /// `/health` is exempt from the IP rate limit tiers
    /// (`server/src/api/rate_limit.rs`), so polling this is safe.
    pub async fn get_chains_health(&self) -> Result<ChainsHealthResponse, ApiError> {
        // "/api/health/chains", not "/health/chains". ApiClient is built with
        // an EMPTY base_url (app/layout.rs), so every path here is absolute and
        // carries its own "/api" - see every other method. Without it the
        // request never reaches the server: nginx serves the SPA fallback for
        // unknown paths, the client parses index.html as JSON, and the panel
        // shows "Parse error: expected value at line 1 column 1".
        self.get("/api/health/chains").await
    }

    /// Whether this server can send outgoing email.
    ///
    /// A server with no SMTP configured accepts email preferences and never
    /// delivers on them, so the UI asks before offering an email channel.
    pub async fn get_email_status(&self) -> Result<EmailStatus, ApiError> {
        self.get("/api/users/me/email/status").await
    }
}
