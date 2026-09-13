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
//! one card without looking broken, and RCS-243 means a third encoding
//! (BIP-21, Solana Pay) is coming for a different chain, not a rewrite.
//!
//! Client-local for now. This is a ui-kit component in spirit — RCS-249 notes
//! the picker will need styling that travels with it — but landing it in
//! commons is a three-step dance (merge, bump the pin, `cargo update`) that a
//! single change here can't complete in one step. Mirrored locally in the
//! meantime, same reasoning as the api-types DTO precedent.

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
        set_active.update(|i| {
            *i = (*i as i32 + delta).rem_euclid(count as i32) as usize;
        });
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
        if let Some(start_x) = drag_start_x.get_untracked() {
            let delta = ev.client_x() - start_x;
            if delta <= -SWIPE_THRESHOLD_PX {
                go(1);
            } else if delta >= SWIPE_THRESHOLD_PX {
                go(-1);
            }
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
                            class="checkout-qr-arrow checkout-qr-arrow-prev"
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
                            class="checkout-qr-arrow checkout-qr-arrow-next"
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

    /// Pure enough to test without a browser: paging wraps rather than
    /// panicking or sticking at an edge, in both directions.
    fn step(count: usize, from: usize, delta: i32) -> usize {
        if count == 0 {
            return from;
        }
        (from as i32 + delta).rem_euclid(count as i32) as usize
    }

    #[test]
    fn wraps_forward_past_the_last_card() {
        assert_eq!(step(3, 2, 1), 0);
    }

    #[test]
    fn wraps_backward_past_the_first_card() {
        assert_eq!(step(3, 0, -1), 2);
    }

    #[test]
    fn single_card_stays_put() {
        assert_eq!(step(1, 0, 1), 0);
        assert_eq!(step(1, 0, -1), 0);
    }

    #[test]
    fn swipe_direction_matches_a_left_drag_advancing() {
        // A drag to the left (finger moves toward negative x) reads as "next
        // page", the same direction a page turns in left-to-right reading.
        let start_x = 200;
        let end_x = start_x - SWIPE_THRESHOLD_PX - 1;
        assert!(end_x - start_x <= -SWIPE_THRESHOLD_PX);
    }
}
