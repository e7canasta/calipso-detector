//! Contract v4: `calipso.scene.v4` and `calipso.events.v4`.
//!
//! The layer-0 transition is its occupancy event and the scene states the
//! machine's mode. What no version changes comes from `common`. Like every
//! version, it is defined once and its JSON Schema comes from these types
//! (rule 13).

pub use super::common::*;

pub use crate::project::v4::cell_of;
pub use scene_contract::Cell;
pub use scene_contract::{
    BedPostureBefore, BedPostureChange, Before, Context, DerivedBody, DerivedEvent, Event, Fact,
    FactBody, MapBody, MapEvent, Occupancy, PartsAfter, PartsBefore, PartsChange, PartsSeen, Room,
    Seen, SensorBody, SensorEvent, Span, Transition,
};
pub use scene_contract::{Envelope, Fv, Mode, Phase, Rbh, SchemaId};

pub use scene_contract::Scene;
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_cell_is_written_in_digits_if_all_counts_are_below_ten() {
        let cell = crate::project::v4::cell_of(Rbh(2, 1, 0), Fv(0, 0), Rbh(1, 1, 1));
        assert_eq!(cell.as_str(), "210:00:111");
    }

    #[test]
    fn a_part_with_two_digit_counts_is_written_as_a_tuple() {
        let cell = crate::project::v4::cell_of(Rbh(12, 0, 0), Fv(13, 0), Rbh(13, 0, 0));
        assert_eq!(cell.as_str(), "(12,0,0):(13,0):(13,0,0)");
        let mixed = crate::project::v4::cell_of(Rbh(9, 1, 1), Fv(10, 0), Rbh(10, 1, 1));
        assert_eq!(mixed.as_str(), "911:(10,0):(10,1,1)");
    }
}
