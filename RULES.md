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
owner.

### 12. Every attention decision has a reason

Each policy returns `Ask(T)` or `Skip(reason)`, and everything goes to the
ledger. The ledger is part of what is published: the consumer knows what was
looked at and what was not, and why.

*Guardian:* the `Decision<T>` type; the contract includes the ledger.

### 13. One contract, one source

The published contract is defined **once**, in `scene_core::contract`, with the
JSON Schema generated from the types. Consumers validate against the schema. An
incompatible change is a major contract version, with an ADR.

*Guardian:* the contract and schema snapshots in CI.

### 21. The core does not panic

The `scene-core` library uses no `unwrap`, `expect`, `panic!`, `unreachable!`,
`todo!` or indexing that could go out of bounds. Every "impossible" case is
modelled in the types or treated as data (a reading of the wrong kind is a
failing sense). Tests are shell and stay outside.

*Guardian:* `#![deny(clippy::{unwrap_used, expect_used, panic, unreachable,
todo, unimplemented, indexing_slicing})]` in the library, with its fixture.

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

### 17. Bisectable commits, daily integration

One logical change per commit. Each commit builds and passes `scripts/check.sh`.
`main` is integrated at least daily and is always deployable.

*Guardian:* `scripts/check.sh` per commit (`git rebase -x scripts/check.sh main`),
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

## Governance

- **Amendments**: a change to this file, with an ADR.
- **Deviations**: a specification that needs to break a rule says so, with a
  reason and a deadline. There are no silent deviations.
- **Review**: at the end of each milestone, every rule is checked for a
  guardian.
