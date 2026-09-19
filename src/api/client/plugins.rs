//! Plugin pages: what to put in navigation, and the page itself.
//!
//! Nothing here knows what any particular plugin does. The server lists what
//! a plugin declared, and the page arrives as a `PageElement` tree this
//! client draws - so a plugin can ship a page to a client that was compiled
//! before the plugin existed.

use super::{ApiClient, ApiError};
use crate::api::{PluginPage, PluginPagesResponse};
use payserver_plugin_api::page::PageElement;

impl ApiClient {
    /// What this server's plugins want listed in navigation.
    pub async fn list_plugin_pages(&self) -> Result<Vec<PluginPage>, ApiError> {
        let response: PluginPagesResponse = self.get("/api/plugins").await?;
        Ok(response
            .plugins
            .into_iter()
            .flat_map(|plugin| {
                plugin.pages.into_iter().map(move |page| PluginPage {
                    plugin_id: plugin.id.clone(),
                    path: page.path,
                    label: page.label,
                })
            })
            .collect())
    }

    /// One plugin page, as a component tree.
    pub async fn get_plugin_page(
        &self,
        plugin_id: &str,
        path: &str,
    ) -> Result<PageElement, ApiError> {
        self.get(&format!("/api/plugins/{plugin_id}/pages/{path}"))
            .await
    }
}
