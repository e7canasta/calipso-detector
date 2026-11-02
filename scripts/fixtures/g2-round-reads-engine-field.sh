# expect-step: the domain without contract
# expect-text: the_round_asks_the_engine_by_name_and_does_not_look_inside
# G2 (rule 11, by Demeter): the round reading a field of the engine, instead of
# asking it by name, must fail its guardian.
cat >> crates/scene-core/src/cycle/sequence.rs <<'RS'

impl Course<'_> {
    #[allow(dead_code)]
    fn frame_seen(&self) -> bool {
        self.engine.frame.is_some()
    }
}
RS
