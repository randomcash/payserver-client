//! QR carousel for the checkout page: one card per address encoding.
//!
//! A wallet wants EIP-681 — it prefills amount and chain, removing the
//! riskiest step in the flow. An exchange withdrawal form wants a bare
//! address and errors on anything else. Neither one is right for every
//! customer, and a failed scan reads as "this site is broken" rather than
//! "wrong card" — so this pages through every encoding worth offering,
//! labelled with what each one is for, rather than picking one.
//!
//! Built to take any number of cards, not a two-card special case: a chain
//! with only one sensible encoding (Tron has no EIP-681 equivalent) offers
//! one card without looking broken, and a third encoding for a different
//! chain (BIP-21, Solana Pay) is an extra card rather than a rewrite.
//!
//! Client-local for now. This is a ui-kit component in spirit, and it will
//! eventually need styling that travels with it — but landing it in commons
//! is a three-step dance (merge, bump the pin, `cargo update`) that a single
//! change here can't complete in one step. Mirrored locally in the meantime,
//! same reasoning as the api-types DTO precedent.

use leptos::ev::KeyboardEvent;
use leptos::prelude::*;

use ui_kit::CopyButton;
use ui_kit::components::crypto::QrCodeDisplay;

/// One renderable encoding of a payment request: what to show, and what it's for.
#[derive(Clone)]
pub struct QrEncoding {
    /// What to tell the customer this card is for, e.g. "Scan with a wallet".
    pub label: &'static str,
    /// The string to encode and to offer for copying — a URI or a bare address.
    /// Never build this if you are not sure it is right: a bare address lets a
    /// customer proceed by hand, where a URI a wallet misparses can send the
    /// wrong amount to the wrong place.
    pub data: String,
}

/// A minimum swipe distance, in pixels, before a drag counts as a page change
/// rather than a tap or a scroll wobble.
const SWIPE_THRESHOLD_PX: i32 = 40;

/// Wrap a page index by `delta`, cycling past either end rather than
/// panicking or sticking at an edge.
fn wrapped_index(count: usize, from: usize, delta: i32) -> usize {
    if count == 0 {
        return from;
    }
    (from as i32 + delta).rem_euclid(count as i32) as usize
}

/// Interpret a horizontal drag as a page step, or `None` if it fell short of
/// `SWIPE_THRESHOLD_PX` and should be read as a tap or scroll wobble instead.
fn swipe_to_page_step(start_x: i32, end_x: i32) -> Option<i32> {
    let delta = end_x - start_x;
    if delta <= -SWIPE_THRESHOLD_PX {
        Some(1)
    } else if delta >= SWIPE_THRESHOLD_PX {
        Some(-1)
    } else {
        None
    }
}

/// A labelled, paged QR display: one card per encoding.
///
/// `encodings` is built once by the caller for the payment option currently
/// selected and handed over whole. Each card is mounted once, with its QR
/// generated from an owned `String` at that point — paging moves which card
/// is visible, it does not rebuild any of them, so swiping through five cards
/// costs one `generate_qr_svg` call each, not one per page turn. A caller that
/// instead selects a *different* payment option passes a new `encodings`, and
/// that does pay for a fresh encode — correctly, since the data changed.
#[component]
pub fn QrPicker(encodings: Vec<QrEncoding>) -> impl IntoView {
    let count = encodings.len();
    let (active, set_active) = signal(0usize);
    let (drag_start_x, set_drag_start_x) = signal(None::<i32>);

    let go = move |delta: i32| {
        if count == 0 {
            return;
        }
        set_active.update(|i| *i = wrapped_index(count, *i, delta));
    };

    let on_keydown = move |ev: KeyboardEvent| match ev.key().as_str() {
        "ArrowLeft" => go(-1),
        "ArrowRight" => go(1),
        _ => {}
    };

    // Pointer events cover touch, mouse and pen with one code path, so a
    // sideways swipe on touch shares its logic with the arrows rather than
    // needing a separate `TouchList` handler.
    let on_pointer_down = move |ev: web_sys::PointerEvent| {
        set_drag_start_x.set(Some(ev.client_x()));
    };
    let on_pointer_up = move |ev: web_sys::PointerEvent| {
        if let Some(start_x) = drag_start_x.get_untracked()
            && let Some(delta) = swipe_to_page_step(start_x, ev.client_x())
        {
            go(delta);
        }
        set_drag_start_x.set(None);
    };

    let cards = encodings
        .into_iter()
        .enumerate()
        .map(|(i, enc)| {
            view! {
                <div
                    class="checkout-qr-card"
                    style:display=move || if active.get() == i { "flex" } else { "none" }
                    role="tabpanel"
                >
                    <span class="checkout-qr-card-label">{enc.label}</span>
                    <QrCodeDisplay data=enc.data.clone() size=250 />
                    <CopyButton text=enc.data.clone() />
                </div>
            }
        })
        .collect_view();

    view! {
        <div
            class="checkout-qr-picker"
            tabindex="0"
            on:keydown=on_keydown
            on:pointerdown=on_pointer_down
            on:pointerup=on_pointer_up
        >
            <div class="checkout-qr-track">
                {(count > 1).then(|| {
                    view! {
                        <button
                            type="button"
                            class="checkout-qr-arrow"
                            aria-label="Previous encoding"
                            on:click=move |_| go(-1)
                        >
                            "‹"
                        </button>
                    }
                })}
                {cards}
                {(count > 1).then(|| {
                    view! {
                        <button
                            type="button"
                            class="checkout-qr-arrow"
                            aria-label="Next encoding"
                            on:click=move |_| go(1)
                        >
                            "›"
                        </button>
                    }
                })}
            </div>
            {(count > 1).then(|| view! {
                <div class="checkout-qr-dots" role="tablist" aria-label="QR encoding">
                    {(0..count).map(|i| view! {
                        <button
                            type="button"
                            role="tab"
                            aria-selected=move || if active.get() == i { "true" } else { "false" }
                            aria-label=format!("Encoding {}", i + 1)
                            class=move || if active.get() == i {
                                "checkout-qr-dot checkout-qr-dot-active"
                            } else {
                                "checkout-qr-dot"
                            }
                            on:click=move |_| set_active.set(i)
                        />
                    }).collect_view()}
                </div>
            })}
        </div>
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // These call `wrapped_index` and `swipe_to_page_step` themselves, the
    // same functions `go` and `on_pointer_up` call - not a copy of the
    // wrapping/threshold math re-derived in the test module. A wrong sign, a
    // flipped comparison or a swapped base in the production code fails
    // these directly, since there is nothing else to fail.

    #[test]
    fn wraps_forward_past_the_last_card() {
        assert_eq!(wrapped_index(3, 2, 1), 0);
    }

    #[test]
    fn wraps_backward_past_the_first_card() {
        assert_eq!(wrapped_index(3, 0, -1), 2);
    }

    #[test]
    fn single_card_stays_put() {
        assert_eq!(wrapped_index(1, 0, 1), 0);
        assert_eq!(wrapped_index(1, 0, -1), 0);
    }

    #[test]
    fn a_left_drag_past_threshold_advances_to_the_next_card() {
        // A drag to the left (finger moves toward negative x) reads as "next
        // page", the same direction a page turns in left-to-right reading.
        assert_eq!(
            swipe_to_page_step(200, 200 - SWIPE_THRESHOLD_PX - 1),
            Some(1)
        );
    }

    #[test]
    fn a_right_drag_past_threshold_goes_back_a_card() {
        assert_eq!(
            swipe_to_page_step(200, 200 + SWIPE_THRESHOLD_PX + 1),
            Some(-1)
        );
    }

    #[test]
    fn a_drag_under_threshold_does_not_page() {
        assert_eq!(swipe_to_page_step(200, 200 - SWIPE_THRESHOLD_PX + 1), None);
        assert_eq!(swipe_to_page_step(200, 200 + SWIPE_THRESHOLD_PX - 1), None);
        assert_eq!(swipe_to_page_step(200, 200), None);
    }

    #[test]
    fn a_drag_exactly_at_the_threshold_pages() {
        assert_eq!(swipe_to_page_step(200, 200 - SWIPE_THRESHOLD_PX), Some(1));
        assert_eq!(swipe_to_page_step(200, 200 + SWIPE_THRESHOLD_PX), Some(-1));
    }
}
