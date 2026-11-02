//! The reception state, as the shell reports it. The core does not infer it:
//! it remembers it, publishes it and announces when it changes (F-15).

use crate::time::SampleRef;

#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct SensorChanged {
    /// The last sample the shell saw, `None` before the first one: v5 names
    /// nothing it did not see.
    pub observed: Option<SampleRef>,
    pub from: SensorState,
    pub to: SensorState,
}

#[derive(Debug, Clone)]
pub(crate) struct Memory {
    state: SensorState,
}

impl Default for Memory {
    fn default() -> Self {
        Self {
            state: SensorState::Connecting,
        }
    }
}

impl Memory {
    pub(crate) fn state(&self) -> SensorState {
        self.state
    }

    /// The shell reports a state; if it changed, that is an event.
    pub(crate) fn report(
        &mut self,
        last: Option<SampleRef>,
        state: SensorState,
    ) -> Option<SensorChanged> {
        let from = std::mem::replace(&mut self.state, state);
        (from != state).then_some(SensorChanged {
            observed: last,
            from,
            to: state,
        })
    }
}

pub(crate) use scene_contract::SensorState;
