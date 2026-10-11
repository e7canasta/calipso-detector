//! The reception state, as the shell reports it. The core does not infer it:
//! it remembers it, publishes it and announces when it changes (F-15).

use crate::time::SampleRef;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct SensorChanged {
    pub at: SampleRef,
    pub from: SensorState,
    pub to: SensorState,
}

#[derive(Debug, Clone)]
pub(crate) struct SensorMemory {
    state: SensorState,
}

impl Default for SensorMemory {
    fn default() -> Self {
        Self {
            state: SensorState::Connecting,
        }
    }
}

impl SensorMemory {
    pub(crate) fn state(&self) -> SensorState {
        self.state
    }

    /// The shell reports a state; if it changed, that is an event.
    pub(crate) fn report(&mut self, at: SampleRef, state: SensorState) -> Option<SensorChanged> {
        let from = std::mem::replace(&mut self.state, state);
        (from != state).then_some(SensorChanged {
            at,
            from,
            to: state,
        })
    }
}

/// The reception state, as the shell reports it. It says nothing about the bed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum SensorState {
    Connecting,
    Streaming,
    Stalled,
    Reconnecting,
}
