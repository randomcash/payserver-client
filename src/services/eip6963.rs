//! EIP-6963 injected-wallet discovery.
//!
//! `window.ethereum` is a single slot: the second extension installed
//! overwrites it, so a customer with two wallets loses access to whichever
//! lost the race. EIP-6963 replaces that with an announce/request event pair
//! — every installed wallet answers on `window`, and the page lists whichever
//! ones actually responded instead of guessing at one.
//!
//! <https://eips.ethereum.org/EIPS/eip-6963>

use leptos::prelude::*;
use send_wrapper::SendWrapper;
use wasm_bindgen::closure::Closure;
use wasm_bindgen::{JsCast, JsValue};
use wasm_bindgen_futures::JsFuture;

const ANNOUNCE_EVENT: &str = "eip6963:announceProvider";
const REQUEST_EVENT: &str = "eip6963:requestProvider";

/// One wallet that answered the discovery request.
#[derive(Clone)]
pub struct DiscoveredWallet {
    pub uuid: String,
    pub name: String,
    pub icon: String,
    /// The EIP-1193 provider object (`{ request(args), ... }`) this wallet
    /// announced. Opaque here — only [`provider_request`] calls into it.
    pub provider: JsValue,
}

/// Listen for EIP-6963 announcements and return the reactive, deduplicated
/// list of wallets discovered so far.
///
/// Dispatches [`REQUEST_EVENT`] once so wallets that already loaded (the
/// common case — the extension injects before this page's own script runs)
/// answer immediately; a page that only listens would miss them. The
/// listener is torn down via `on_cleanup`, so calling this once per render of
/// a dynamic view (checkout's chain selector rebuilds its whole subtree on
/// every switch) does not accumulate listeners.
pub fn discover_wallets() -> ReadSignal<Vec<DiscoveredWallet>> {
    let (wallets, set_wallets) = signal(Vec::<DiscoveredWallet>::new());

    let Some(window) = web_sys::window() else {
        return wallets;
    };

    let closure = Closure::<dyn FnMut(web_sys::Event)>::new(move |event: web_sys::Event| {
        let Some(event) = event.dyn_ref::<web_sys::CustomEvent>() else {
            return;
        };
        let Some(wallet) = parse_announcement(&event.detail()) else {
            return;
        };
        set_wallets.update(|list| {
            if !list.iter().any(|w| w.uuid == wallet.uuid) {
                list.push(wallet);
            }
        });
    });

    if window
        .add_event_listener_with_callback(ANNOUNCE_EVENT, closure.as_ref().unchecked_ref())
        .is_ok()
    {
        // `on_cleanup` requires `Send + Sync`, which a JS closure never is.
        // `SendWrapper` is sound here because WASM is single-threaded — there
        // is no other thread for it to be sent to.
        let cleanup_window = window.clone();
        let guard = SendWrapper::new((closure, cleanup_window));
        on_cleanup(move || {
            let (closure, window) = guard.take();
            let _ = window.remove_event_listener_with_callback(
                ANNOUNCE_EVENT,
                closure.as_ref().unchecked_ref(),
            );
            drop(closure);
        });
    }

    if let Ok(request) = web_sys::CustomEvent::new(REQUEST_EVENT) {
        let _ = window.dispatch_event(&request);
    }

    wallets
}

fn parse_announcement(detail: &JsValue) -> Option<DiscoveredWallet> {
    let info = js_sys::Reflect::get(detail, &"info".into()).ok()?;
    let provider = js_sys::Reflect::get(detail, &"provider".into()).ok()?;
    if provider.is_undefined() || provider.is_null() {
        return None;
    }

    let uuid = js_sys::Reflect::get(&info, &"uuid".into())
        .ok()?
        .as_string()?;
    let name = js_sys::Reflect::get(&info, &"name".into())
        .ok()?
        .as_string()?;
    let icon = js_sys::Reflect::get(&info, &"icon".into())
        .ok()
        .and_then(|v| v.as_string())
        .unwrap_or_default();

    Some(DiscoveredWallet {
        uuid,
        name,
        icon,
        provider,
    })
}

/// Call `provider.request({ method, params })` — the single entry point
/// every EIP-1193 provider exposes, announced wallets included.
pub async fn provider_request(
    provider: &JsValue,
    method: &str,
    params: &JsValue,
) -> Result<JsValue, String> {
    let request = js_sys::Object::new();
    js_sys::Reflect::set(&request, &"method".into(), &method.into())
        .map_err(|e| describe_js_error(&e))?;
    js_sys::Reflect::set(&request, &"params".into(), params).map_err(|e| describe_js_error(&e))?;

    let request_fn = js_sys::Reflect::get(provider, &"request".into())
        .ok()
        .and_then(|f| f.dyn_into::<js_sys::Function>().ok())
        .ok_or_else(|| "wallet provider has no request()".to_string())?;

    let result = request_fn
        .call1(provider, &request)
        .map_err(|e| describe_js_error(&e))?;

    JsFuture::from(js_sys::Promise::from(result))
        .await
        .map_err(|e| describe_js_error(&e))
}

/// Best-effort human message from a rejected EIP-1193 request — a thrown JS
/// error, an `{code, message}` object (a decline is `code: 4001`), or a bare
/// string.
fn describe_js_error(value: &JsValue) -> String {
    if let Some(s) = value.as_string() {
        return s;
    }
    js_sys::Reflect::get(value, &"message".into())
        .ok()
        .and_then(|m| m.as_string())
        .unwrap_or_else(|| "wallet request failed".to_string())
}
