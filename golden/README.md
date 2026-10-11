# Golden scenes

Each file is a scene: a night in spans, in the same language as the Gherkin
tables of `features/occupancy` and `features/attention`. The `golden` crate reads it
([language](../crates/golden/src/language.rs)).

```markdown
---
format: 1           # required: the language version
cadence_s: 1        # source seconds between samples
dwell_s: 2          # the dwell the expectations hold with
---

# Title

| samples | on floor | where    | in bed | occupants | bed            |
|---------|----------|----------|--------|-----------|----------------|
| 1-5     | 2        | door     | 0      | 2         | empty          |
| 6-10    | 1        | near bed | 1      | 2         | occupied, seen |
```

- What is seen: `samples`, `on floor`, `where` (`door` or `near bed`), `in bed`
  (a number or `0 and 1`), and optionally `reinforcement in bed` and `faces`
  (`on subject`, `at headboard`, `weak`).
- What is believed, optional: `occupants` and `bed` (`empty`, `occupied`,
  `occupied, seen`, `occupied, held`, `unknown`), at the end of each span.
- `where` can also be `N near bed`: of those seen on the floor, N are next to
  the bed and the rest at the door.
- `senses` (property): what the expectations assume (`detect`, `reinforce`,
  `faces`); with reinforcement or faces, run with `config/scene-r6.toml`.
- The night starts at sample 1 and has no gaps.

## Running them

```sh
calipso-detector --config config/scene.toml --scene golden/s1-staff-put-to-bed.md --fast --jsonl /tmp/s1.jsonl
calipso-distill /tmp/s1.jsonl --dwell 2      # a published night, back into a scene
```

In CI, every scene runs through the real detector and must match its
expectations, be identical to the direct loop and survive the distill round
trip. S1–S8 also have their scenario in the features, and a test checks that
both say the same.
