//! The burst of crops a question sends (white book, volume III, ch. 12): the
//! crops kept for one ticket, in order, until its question goes out. The
//! transport knows nothing of it — this is the store, testable without NATS.

/// The crops kept for the burst being collected, by its ticket's id. One
/// burst at a time: a crop kept for another ticket drops the burst in
/// progress, and a question asked for another ticket takes none of it — a
/// burst never mixes tickets.
#[derive(Debug, Default)]
pub(crate) struct CropBurst {
    for_ticket: Option<(u64, Vec<Vec<u8>>)>,
}

impl CropBurst {
    /// Keeps one more crop for `ticket`'s burst, in the order they arrive.
    pub(crate) fn keep(&mut self, ticket: u64, jpeg: Vec<u8>) {
        match &mut self.for_ticket {
            Some((id, kept)) if *id == ticket => kept.push(jpeg),
            _ => self.for_ticket = Some((ticket, vec![jpeg])),
        }
    }

    /// The burst kept for `ticket`, and it is spent: the question takes it.
    /// Another ticket's burst is dropped, not sent (one burst at a time).
    pub(crate) fn take(&mut self, ticket: u64) -> Vec<Vec<u8>> {
        match self.for_ticket.take() {
            Some((id, kept)) if id == ticket => kept,
            _ => Vec::new(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn jpeg(n: u8) -> Vec<u8> {
        vec![n]
    }

    #[test]
    fn the_burst_keeps_the_crops_in_order_and_is_spent_once() {
        let mut burst = CropBurst::default();
        burst.keep(1, jpeg(10));
        burst.keep(1, jpeg(11));
        assert_eq!(burst.take(1), vec![jpeg(10), jpeg(11)]);
        assert!(burst.take(1).is_empty(), "the burst was spent");
    }

    #[test]
    fn one_burst_at_a_time_a_crop_of_another_ticket_drops_the_burst() {
        let mut burst = CropBurst::default();
        burst.keep(1, jpeg(10));
        burst.keep(2, jpeg(20));
        assert_eq!(
            burst.take(2),
            vec![jpeg(20)],
            "only the newest ticket's burst stands"
        );
        let mut burst = CropBurst::default();
        burst.keep(1, jpeg(10));
        burst.keep(2, jpeg(20));
        assert!(burst.take(1).is_empty(), "ticket 1's burst was dropped");
    }

    #[test]
    fn a_question_of_another_ticket_takes_nothing_and_drops_the_burst() {
        let mut burst = CropBurst::default();
        burst.keep(1, jpeg(10));
        assert!(burst.take(2).is_empty());
        assert!(burst.take(1).is_empty(), "the burst was dropped, not kept");
    }
}
