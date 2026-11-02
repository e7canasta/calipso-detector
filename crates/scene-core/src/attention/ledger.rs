//! The ledger: what was looked at, what was not and why. It is published
//! language (rule 12): the contract re-exports these types. Nothing here decides
//! anything; the decisions about what to look at live in `attention`.

pub(crate) use scene_contract::{Attention, Look, LookOutcome, LookResult, SkipReason};
