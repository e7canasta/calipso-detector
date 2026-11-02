# Rules

What is not negotiable in `calipso-detector`. Specifications, plans and code
are subordinate to these rules. **Every rule has a guardian**: a type, a lint,
a test or a review that enforces it. A rule without a guardian is an
aspiration, not a rule.

The code cites them by number ("rule 8"). The numbering is stable: a rule is
never renumbered, only amended or retired.

## Part I — The domain

### 1. Not seeing is not absence

An instrument failure, an ambiguous gap or a missing sample is **never** proof
of absence. Only a **strict zero** (candidates discarded with an explicit
threshold) may take a retained belief to empty.

*Guardian:* the evidence type tells the four zeros apart; occlusion and failure
scenarios.

### 2. Nothing is current without an observation in this turn

Every published claim carries its provenance: `Current`, `Retained { since }`
or `Stable { since }`. What is retained keeps its origin; enrichment has its
own provenance.

*Guardian:* an invariant checked on every turn of every scenario: every
`Current` has a reading in the turn that produced it.

### 3. One sample, one scene

A scene describes **a single** sample and is projected **once**, when the turn
closes. The projection does not decide and is never read back as input.

*Guardian:* `project` receives the conclusion by value and has no access to
`Belief`.

### 4. Events have an order and a cause

Within a turn, events go out in a fixed order documented in the contract. A
**domain event** is emitted by the aggregate that changed, with its cause; a
**derived event** compares projections and says so.

*Guardian:* an order property over every scenario; the event type tells domain
and derived apart.

### 5. The freshest wins

Samples are not queued: the mailbox has one slot.

*Guardian:* the `ingest::Mailbox` tests.

### 6. A sense returns everything it sees

An adapter does not filter by policy. Evidence decides what counts.

*Guardian:* the `sense` adapters receive no policies.

### 7. No images, ever, outside the verification capture

The system observes people in their beds. **No published format, recording,
log or artefact of the detector keeps pixels**: only boxes, keypoints, depths
and decisions. The one exception is the **verification capture**: the crops of
the bed sent to the verifier, kept by the verifier service (never the
detector) only when its profile enables it, by stream and day, with a
retention, to distil small models (ADR 2026-10-13, *La verificación por VLM,
con captura*). Using it in a residence needs the site's consent.

*Guardian:* the published and recorded formats have nowhere to put pixels; the
detector only encodes a crop in memory to send it to the verifier; the capture
is off unless the verifier's profile enables it.

## Part II — The architecture

### 8. The core does no IO

`scene-core` believes, decides where to look and projects. It sees no pixels,
calls no models, reads no clocks and touches no network, disk or threads.

*Guardian:* `scripts/check.sh` (the core's dependency tree has no `image`,
`ort`, `retina`, `tokio`, `async-nats`) and clippy's `disallowed-types` and
`disallowed-methods` (`std::fs`, `std::net`, `std::thread`, `Instant`,
`SystemTime`) with `-D warnings`.

### 9. The domain only knows source time

Dwell, provenance, motion **and retries** are measured in video time. The core
receives no other clock. That is why a recorded night replays the same at any
speed.

*Guardian:* `Input` carries only `SampleRef` as time; replay at another cadence
(the metamorphic tests).

### 10. The loop is an explicit protocol

The core asks, the shell looks and answers, and the core decides the next step.
A failure is data. The shell drives a turn in one call (`drive`, or `cancel`
on shutdown): there is no order to get wrong, and no turn is abandoned.

*Guardian:* the API offers no other way through; `#[must_use]` and an
assertion in `Drop`.

### 11. `cycle` only sequences

The order of a turn lives in one place with no domain rules. A rule in `cycle`
is in the wrong place.

*Guardian:* `cycle` never asks which senses are configured: it hands the
whole `Policy` to the module that decides (`scripts/check.sh`); review by its
owner. And the sequence does not reach into the engine: outside
`cycle/engine.rs`, no file of `cycle/` reads a field of `Engine`; it asks the
engine, which answers with its own slices (`tests/guardians.rs`, with its
mutation). `Course` borrows the engine, not the `Monitor`, so the engine cannot
read the chronicle (P4) by construction.
*Amended:* ADR 2026-11-02, la secuencia ordena, el motor responde.

### 12. Every attention decision has a reason

Each policy returns `Ask(T)` or `Skip(reason)`, and everything goes to the
ledger. The ledger is part of what is published: the consumer knows what was
looked at and what was not, and why.

*Guardian:* the `Decision<T>` type; the contract includes the ledger.

### 13. One contract, one source

The published contract is defined **once**, in `scene-contract` (the wire: data
types only), with the JSON Schema generated from the types. The core re-exports
nothing of it except through `project`; consumers depend on `scene-contract`
and never on the core to read the wire. An incompatible change is a major
contract version, with an ADR.

What a rule or a machine of the core computes is the core's own type, and
`project` copies it to the wire. From `scene-contract` the core uses only the
**shared kernel**: the instant, the zone and the count, the provenance, the
body's points, the reception and frame states the shell reports, and the
attention ledger, which is published as it is written (rule 12).

*Guardian:* the contract and schema snapshots in CI; `contract-depends-on-core`;
`tests/guardians.rs` reads the shared kernel's list and fails on any other
`scene_contract` name outside `project/` and `cycle/`.
*Amended:* ADR 2026-11-02, crates del lenguaje y del cable (S7); ADR
2026-11-02, el dominio es dueño de lo que decide.

### 21. The core does not panic

The `scene-core` library uses no `unwrap`, `expect`, `panic!`, `unreachable!`,
`todo!` or indexing that could go out of bounds. Every "impossible" case is
modelled in the types or treated as data (a reading of the wrong kind is a
failing sense). Tests are shell and stay outside.

*Guardian:* `#![deny(clippy::{unwrap_used, expect_used, panic, unreachable,
todo, unimplemented, indexing_slicing})]` in the library, with its fixture.

### 23. Space has one owner

`room` answers every question of space (in bed or on the floor, near the bed,
where a head or a part falls against the bed) with named facts, never with a
plane or a polygon. The camera's geometry (`Frontal`, `TwoPlanes`) is chosen
there and nowhere else. `room` remembers nothing and asks nothing of the
layers above it: what a layer concludes from space (where the head is, the
posture) belongs to that layer.

*Guardian:* `scripts/check.sh`: the bed's plane (`BedPlane`) only in `map`
and `room` (fixture `space-outside-room`), and `room` imports no layer above
it (fixture `room-reads-a-layer`).

### 24. The business is written in the code

What the engine decides is code that names the domain: the conservation
rules, the order of the turn, the layers and their tables, the heads of the
second level. Data configures numbers, switches a capability on or off and
describes the site; it never expresses a rule, a step or an order. The golden
tables are **oracles**: the code decides and the table checks it; nothing
interprets a table at run time. Composition is of concrete, named parts,
assembled once (`Monitor::new`, `main.rs`), never a graph read at run time.

A look, a layer or a head is added by name: a variant, a function, a line in
`Course::next`, a row set in its table. There is no generic step, node or
context that every piece reads from.

*Guardian:* `tests/guardians.rs`: the core puts no decision behind a dynamic
object (`Box<dyn …>`, `Vec<Box<dyn …>>`, `Rc`/`Arc<dyn …>`; local `&dyn Fn`
predicates are allowed), no string-keyed map in the policy or the evidence, and
does not read the order of the turn from data. Each with its mutation. Review
for the rest.
*Why:* ADR 2026-11-02, el negocio se escribe en el código: generic workflows,
graphs and attribute maps turn the business into something nobody can read or
change.

## Part III — How we work

### 14. Specification before code

No behaviour enters without an approved specification: **what and why**, in the
domain's language. The plan says how; the tasks are the series of commits.

*Guardian:* review.

### 15. The specification is executable (BDD)

Each specification starts from a user story and a use case. Each accepted path
of the use case is a **Gherkin scenario** in `features/`, in the glossary's
language, which the BDD harness runs against the real loop with scripted
senses: no images, no models. A scenario that does not run is not specified.

*Guardian:* the BDD harnesses run every `.feature` and fail on an undefined
step or a skipped scenario.

### 16. Red, green, refactor

First the scenario fails, then it passes with the simplest thing, then it is
cleaned up with the net in place.

*Guardian:* in a series, the scenario and the test come before the
implementation.

### 17. Bisectable commits, verified at the size of the change

One logical change per commit. Each check runs at its moment, and no larger
than the question it answers:

| Level | When | What |
| --- | --- | --- |
| Working | every change | the crate that changed, or one feature or test module: `cargo test -p scene-core --test bdd -- --name '…'`, `cargo test -p detector --test it nats::` |
| Quick | every commit | `scripts/quick.sh`: format, then clippy and tests of the crates that changed and of those that depend on them |
| Full | every version: a branch into `main`, a cut | `scripts/check.sh`, the whole gate; `scripts/check-guardians.sh` when the gate or a lint changed |

`main` is integrated at least daily and always passes the full gate. A commit
inside a branch that breaks only what the full gate sees (a license, another
crate's `#[ignore]`d test) is fixed before the branch goes into `main`.

*Why:* a full gate per commit cost five minutes each and turned a five-minute
change into twenty (2026-10-31); the quick level costs about ninety seconds
for a change in the core.

*Guardian:* `scripts/check.sh` on the branch head before it goes into `main`,
and `scripts/check-guardians.sh` proves each gate fails when its rule is broken.

### 18. Simple design, YAGNI

Beck's four rules: passes the tests, reveals intent, no duplication, fewest
elements. No traits, frameworks or crates without a real second use.

*Guardian:* review.

### 19. Owners

Everyone may touch everything; each subsystem has an owner who reviews. A change
that crosses a contract needs a review from both sides.

*Guardian:* `CODEOWNERS`.

### 20. The ubiquitous language rules

Names in the code are the glossary's. A new term enters the glossary first and
the code after. The glossary lives in `crates/scene-core/DESIGN.md`.

*Guardian:* review.

### 22. One build, reused

Work happens in the checkout, with its `target/` warm: a branch is switched
there, not copied. A tool that needs a copy of the workspace (the guardians,
the loop) keeps it at a fixed path and shares one target, because cargo hashes
a workspace crate with its path and a new directory is a new build that
nothing reuses or removes. Debug builds carry line tables only.

*Why:* temporary copies and full debug info grew `target/` to 54 GB, and a
cold build costs minutes every time (2026-10-31).

*Guardian:* `[profile.dev]` in `Cargo.toml`; review of every script that sets
`CARGO_TARGET_DIR` or copies the workspace.

## Governance

- **Amendments**: a change to this file, with an ADR.
- **Deviations**: a specification that needs to break a rule says so, with a
  reason and a deadline. There are no silent deviations.
- **Review**: at the end of each milestone, every rule is checked for a
  guardian.
