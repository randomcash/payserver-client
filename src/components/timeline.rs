//! What an Activity timeline dot is saying.
//!
//! The dot is the only thing on these rows carrying colour, so it is the whole
//! signal: green means it happened, amber means it is still happening, red
//! means it happened and it went wrong.
//!
//! It used to be possible to render a row with no state at all - the class was
//! written inline per row, and four of the eight rows across the payment and
//! invoice pages simply omitted the modifier. Those fell back to
//! `--border-color`, so "Payment detected" and "Invoice created" rendered grey
//! forever, on events that had definitively happened. A merchant reading the
//! timeline saw a completed payment with a live dot above a dead one.
//!
//! Going through [`TimelineState`] removes that option: every row picks a state,
//! because there is no longer a way to write a row without one.

/// What a row on the timeline is saying about its event.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TimelineState {
    /// It happened, and that is fine. Green.
    Done,
    /// It has not happened yet and is expected to. Amber, pulsing.
    Pending,
    /// It happened and it was bad. Red.
    Failed,
}

impl TimelineState {
    /// The full class list for the row, modifier included.
    ///
    /// Returns both classes rather than just the modifier so a caller cannot
    /// write `class="timeline-item"` and silently get the unstyled default -
    /// which is the bug this type exists to remove.
    #[must_use]
    pub fn row_class(self) -> &'static str {
        match self {
            Self::Done => "timeline-item timeline-item-success",
            Self::Pending => "timeline-item timeline-item-pending",
            Self::Failed => "timeline-item timeline-item-error",
        }
    }
}

/// The state of one payment, for its row on a timeline.
///
/// A reorged payment is `Failed` whether or not it had confirmed: the chain
/// rolled it back, so the money is not there. That check comes first for
/// exactly that reason - a confirmed-then-reorged payment must not read green.
#[must_use]
pub fn payment_state(reorged: bool, confirmed: bool) -> TimelineState {
    if reorged {
        TimelineState::Failed
    } else if confirmed {
        TimelineState::Done
    } else {
        TimelineState::Pending
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_confirmed_payment_is_done() {
        assert_eq!(payment_state(false, true), TimelineState::Done);
    }

    #[test]
    fn an_unconfirmed_payment_is_still_pending() {
        assert_eq!(payment_state(false, false), TimelineState::Pending);
    }

    #[test]
    fn a_reorged_payment_has_failed_even_though_it_confirmed() {
        // The case the ordering exists for. A payment can confirm and then be
        // rolled back by a reorg; reading that as Done would show a merchant
        // money they do not have.
        assert_eq!(payment_state(true, true), TimelineState::Failed);
        assert_eq!(payment_state(true, false), TimelineState::Failed);
    }

    #[test]
    fn every_state_carries_the_base_class_and_a_modifier() {
        // The bug: a row rendered with only `timeline-item` falls back to the
        // grey default and says nothing. No state may produce that.
        for state in [
            TimelineState::Done,
            TimelineState::Pending,
            TimelineState::Failed,
        ] {
            let class = state.row_class();
            assert!(
                class.starts_with("timeline-item "),
                "{state:?} must keep the base class: {class}"
            );
            assert!(
                class.split_whitespace().count() == 2,
                "{state:?} must carry exactly one modifier: {class}"
            );
            assert_ne!(
                class, "timeline-item",
                "{state:?} must not fall back to the unstyled default"
            );
        }
    }

    #[test]
    fn the_three_states_are_visually_distinct() {
        // Three different things to say, three different classes. If two ever
        // collapse to the same one the dot stops carrying information.
        let classes = [
            TimelineState::Done.row_class(),
            TimelineState::Pending.row_class(),
            TimelineState::Failed.row_class(),
        ];
        let unique: std::collections::HashSet<_> = classes.iter().collect();
        assert_eq!(
            unique.len(),
            3,
            "states must not share a class: {classes:?}"
        );
    }
}
