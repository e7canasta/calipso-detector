# scene-core design

> The engine's reference: what it is today, what it guarantees and what each
> piece does. The why and the path are in the books, and the decisions in the
> ADRs, both in the `calipso-docs` repository; the rules are in `RULES.md` at
> the repository root. If this document and the code disagree, one of them has
> a bug.

## 1. What it is and what it is not

`scene-core` is the detector's core: **it believes on partial evidence, decides
where to look and projects what it believes.** It is a deterministic function
of `(Policy, SiteMap, Input*, Reading*)`: with the same inputs it gives the same
scene and the same facts, always.

It sees no pixels, calls no models, reads no clocks, touches no network, disk or
threads. All of that belongs to the **shell** (the `detector` crate): it
decodes, runs the senses and publishes. The core asks for looks and reads what
comes back.

```text
          shell (detector)                     core (scene-core)
  ┌──────────────────────────┐  Input / Reading  ┌─────────────────────────┐
  │ decode · senses (ONNX)   │ ────────────────► │ Monitor / Turn          │
  │ NATS · clock             │ ◄──────────────── │   believe, decide,      │
  └──────────────────────────┘  Probe,           │   project               │
                                Output           └─────────────────────────┘
```

### The context map

`scene-core` is **one bounded context**, "the occupancy of the room", with its
aggregates (`Belief`, `Map`, `SensorMemory`, `Chronicle`). Outwards it speaks a
versioned **published language** (`contract::v4`) and `project`
is its anti-corruption layer: the domain thinks in its own vocabulary and only
`project` translates it.

```text
 sense (perception, ONNX) ──ports──► detector (shell: Open Host Service)
 ingest (transport)       ─────────►      │  publishes
                                          ▼
                scene-core ── published language: contract::v4
                                          │
          other repositories: calipso-golden (scenes and replay),
                              calipso-rerun-bridge, calipso-shark
```

The vocabulary the domain and the contract share is **owned by the domain**
(`time`, `zone`, `body`, `provenance`, `look`, `evidence`, `attention`, `map`,
`sensor`, `refine`) and each contract version re-exports it. That way a new
version shares it without depending on the previous one, and retiring one does
not touch the core.

### Principles

The code cites these by their id, as it cites the rules of `RULES.md` by
number.

| Id | Principle | Source |
|---|---|---|
| P1 | **The core is Markov.** The machine keeps only what it needs to decide the next transition and the identity of what is believed (which box is held). No "since when" | Automata theory |
| P2 | **The machine does not emit: it returns.** `step(state, input) → (state, transition)`. The transition is a value with `before`, `seen`, `after` and the facts | Chassaing, *Decider* (2021) |
| P3 | **Mechanism, not policy.** The core offers the point of observation; what is recorded, published or aggregated is decided by whoever observes | The Linux kernel's tracepoints |
| P4 | **History is a fold.** "Since when" the room's name and the bed's kind hold is a fold of concluded turns, kept outside the engine in `chronicle`; `project` translates it. The episode and the counts of a night stay outside the core | Fowler, *Event Sourcing* (2005) |
| P5 | **Every observer sees the same, in order, and none writes to the core.** Recorder, publisher, projections and TUI are a `tee` of one sequence | Unix pipelines |
| P6 | **The transition carries its state** (`before`, `seen`, `after`): whoever receives it does not need to ask the core | Fowler, *event-carried state transfer* (2017) |
| P7 | **The engine does one thing well.** It computes inside only what it uses to decide where to look | Unix philosophy |
| C1 | **Consumers are not broken.** A published version keeps going out unchanged, byte for byte, while someone reads it | The kernel's "we don't break userspace" |
| C2 | **Expand, migrate, contract.** A new version goes out next to the old one; consumers migrate one at a time; the old one is retired | Fowler, *Parallel Change* |
| C3 | **One version, one source.** Each version is defined once (`contract::v4`) and its JSON Schema comes from the types (rule 13) | |
| C4 | **Every version comes out of the same turn.** `project` translates the same domain values into each version in the same turn: they cannot tell different things | |
| C5 | **A test proves it.** While two versions are published together, a test lowers the new one to the old one on every turn of the suites; it is retired with the old version | Metamorphic testing |

A published version is frozen (C1), so its types are deliberately **not**
`#[non_exhaustive]`: a consumer may `match` them exhaustively, and the compiler
tells them what a new version changed when they move to it.

## 2. Invariants

What no change may break. Each one has someone watching it.

| Invariant | Rule or principle | Who watches it |
|---|---|---|
| No IO: no clocks, files, network or threads | 8 | `clippy.toml`, `scripts/check.sh` (dependency tree) |
| Only source time; never compared across sessions | 9 | `SampleRef::secs_since` returns `None` across sessions |
| The loop is a protocol: a turn is not abandoned | 10 | `Turn` is `#[must_use]`, holds the belief exclusively and only `drive` or `cancel` close it |
| `cycle` only sequences; each decision belongs to another module | 11 | `scripts/check.sh` (cycle consults no policy); review; the order lives in `Course::next` |
| Every attention decision has a reason | 12 | `SkipReason` on every no |
| One contract, one source: the JSON Schema comes from the types | 13 | the contract snapshots |
| No panics: no `unwrap`, `expect` or indexing | 21 | `#![deny(...)]` in `lib.rs` |
| Layer 0 decides as the golden table does | P2 | `belief/occupancy/machine/table.rs` |
| Layer 1 decides as its golden table does (`contract/bed-posture-table.txt`) | P2 | `belief/bed_posture/rules/table.rs` |
| The parts decide as their golden table does (`contract/parts-table.txt`) | P2 | `belief/parts/rules/table.rs` |
| The second level's arbiter decides as its golden table does (`contract/second-level-table.txt`) | P2 | `refine/verdict.rs` |
| V settles an answer, or its expiry, as its golden table does (`contract/verification-table.txt`) | P2 | `verification/table.rs` |
| The domain does not import the contract: it decides with its own vocabulary, and only `project` and `cycle` (the `Output`) name `contract`. The chronicle remembers the domain. What no version changes is projected into `contract::common`, without naming a version | P3, P4, C2 | `scripts/check.sh` (the domain without contract) |
| Space has one owner: the bed's plane lives only in `map` and `room` | 23 | `scripts/check.sh` (space has one owner), with its fixture |
| The room does not depend on the layers above it: it answers with the site, the policy and the shared vocabulary, and whoever concludes something asks it | 23, ADP (Martin) | `scripts/check.sh` (the room asks nothing above it), with its fixture |
| Determinism: a replay gives the same. Recording is always outside the detector, so a real night is replayed from what was published: its counts and layers come back, the verifier's answers do not (they are published as provenance, `held.check`, and the distilled scene leaves them out) | 8, 9 | golden scenes and the detector's replay |

## 3. The loop: one turn per sample

The public API is the loop protocol: `Monitor::handle` opens a `Turn` per
input and `Turn::drive` runs it to the end. For each `Probe` the shell looks
and returns a `Reading` (or a `Failure`) and how long it took; the turn closes
with the `Output` (`at`, `scene`, `events`). `Turn::cancel` closes it as a
shutdown. There is no other way through: the shell has nothing to misuse.

`Monitor` composes three pieces that do not mix (P4, P5): the
**engine** (`Engine`: policy, map, belief, sensor), which decides with what it
sees; the **chronicle** (`chronicle::Chronicle`), which remembers the
domain; and the last scene, for republishing. Closing splits the turn in
two phases: the engine returns a `Conclusion` (an `Observation` or what to
republish) without reading the chronicle, and `Monitor::project` notes it in
the chronicle and translates it to the contract.

```text
 Engine ──► Conclusion ──► Chronicle::record ──► Chronicled ─┐
                │                                            ├──► project::v4::observed ──► Output
                └────────────────────────────────────────────┘
```

`project::v4::observed` translates the observation into the published
version. What no version changes (the evidence, the bed, the subject, how
republishing degrades) lives in `project` itself, as `contract::common` does
for the types. `Output` carries the turn's `scene` and `events` in the
published version. A version published next to it (C2) adds its own module,
and a test lowers one to the other (C5).

```text
 look ──► evidence ──► belief ──────────────► refine ──► project ──► Output
 ask      admit        believe: layer 0       second     translate
                       (occupancy), subject   level      to the contract
            ▲                │
            └── attention ◄──┘   at each point: what else to look at? reads, never writes
```

One turn, in order: detect over the whole image, or over the focus while the
resident is alone in bed (§3.1); widen to the whole image if the focus did
not confirm?; admit bodies per zone; reinforce a loss in bed?; faces?; a
doubtful body's pose, for the room to place it (§3.2)?; believe (stabilize, conserve, derive the subject);
calibrate, pose (and where the head is), depth?; conclude the enrichment; the
second level reads it and layer 1 steps (§5, layer 1); a doubt worth the slow
verifier? (a doubtful digit first, then the held body; the answer arrives
between turns and is settled at the next sample's turn); project the scene and
the facts. The order lives in a single place: `Course::next` (`cycle/sequence.rs`).

### 3.1 The focus: a strategy of the main detector

While layer 0 believes the resident is alone in bed (`Room::Alone(InBed(_))`)
and nothing is stabilizing, the main detector looks at the **focus**: the site
map's, or the bed grown by `FocusPolicy::pad`. The same model gets more pixels
per body where the resident is lost to a blanket. Layer 0 does not know it
exists:

- **The focus only confirms.** It confirms what is believed (one in bed if
  seen; nobody, or the one who reappears, if held) with nobody on the floor.
  Anything else and the same turn looks at the whole frame before believing
  (`attention::widen`): layer 0 never receives a reading of the focus that
  did not confirm.
- **The sentinel.** The whole frame is looked at again every
  `wide_every_secs` of source time (`attention::Lens`). Kept no longer than
  staff take to walk from the door to the bed, whoever comes in is seen on
  the floor first and layer 0 knows where they came from (rule 6). Someone
  far from the bed is seen up to that late; nobody is invented or lost.
- **The wire does not change.** The ledger already publishes the region of
  each `detect` (`null`: the whole frame); a widened turn has two.

```text
Primary(focus | whole) → [Widen] → [Reinforce] → [Faces] → [PoseBodies] → believe → …
```

### 3.2 The doubtful body: its pose before believing

A body the box alone cannot place is **doubtful**, and only the room says
which (`Room::doubtful`): in two planes with the knee in the profile, one left
on the floor by its support with enough of its box over the mattress (sitting
on the foot of the bed, or standing at it). Frontal never has one, so the step
does not exist for it and nobody outside `room` asks which piece it got.

- `attention::pose_bodies` asks for the pose of the doubtful body with the
  most box over the mattress, one per turn (`Probe::Pose` with
  `PosePurpose::Body`, ledger kind `pose_body`).
- `evidence::pose_body` hands the pose to the room (`Room::posed`): a knee
  bent to `knee_bent_deg` or less moves the body from the floor to the bed. A
  failing pose, or no knee in sight, leaves it on the floor (rule 1).
- The body published carries `knee_deg`. Layer 0 decides where the pelvis is;
  sitting stays layer 1's (white book, volume VII, ch. 15).

## 4. Mechanism and policy

The core is mechanism; policy comes from outside, and so does observation.

- **Policy** (`policy`): dwell, admission, optional senses. It comes from the
  detector's profiles; the core reads it from nowhere.
- **Mechanism**: how to stabilize, which rules conserve people, what to ask to
  look at. It is core code and is not configurable.
- **Observation**: what is published, recorded or aggregated over a night. The
  observers decide it, outside the core (P3, P5).

It is the rule of the kernel's tracepoints: the core marks the point, whoever
observes decides what to do with it.

## 5. The machines: layer 0 and layer 1

Two machines with the same shape, one under the other. Layer 0 is where the
bodies are; layer 1, for the resident left lying, whether they still are.
Layer 1 reads layer 0 and never changes it.


How many are on the floor, how many in bed and who is still in bed unseen. It
lives in `belief/occupancy/` and does not know the second level exists.

### The named state (`room.rs`)

```text
Room
├── Empty                         000
├── Alone
│   ├── Out                       100
│   └── InBed
│       ├── Seen                  110
│       └── Held                  111   not seen, did not leave: still there (rule 1)
└── Accompanied(Crowd)            two or more: 200, 210, 211, …
```

The counts are the data; `Room::of` gives them their name, without loss. The six
conservation rules decide over the counts and scale to any number of people.

### How a change is decided

```text
Bodies ──► Span ──(Before, Seen)──► settle ──[Movement]──► Machine::apply ──► Transition
           did the new count         what         (evolve per    identity and     before, seen,
           hold?                     explains it?  movement)      facts            after, facts
```

- `Span` (`span.rs`): the span since the last stable count. It uses the
  `Stabilizer` (`stabilizer.rs`) in source time and delivers two columns of the
  table: what we believed (`Before`) and what we saw (`Seen`).
- `settle` and `evolve` (`conservation.rs`): a pure *decider*. `settle` decides
  the ordered list of movements; `evolve` says how each one moves the counts. No
  state, no events, no boxes.
- `Machine` (`machine.rs`): folds the movements and gives them identity (which
  box stays held, where the subject came from). `Machine::step` **returns**
  `Option<Transition>`: the table row (`before`, `seen`, `after`) and the facts
  in order, each with the occupants it leaves. It neither emits nor publishes
  (P2).
- Facts (`Fact`) are domain facts and have no cause: adoption at startup is its
  own fact, `Fact::Adopted`, and `project` decides it is published as
  `lay_down` with cause `baseline`.

### What `Machine` keeps

Only what it needs to decide the next transition and the identity of what is
believed (P1):

| Field | What it is | What it decides |
|---|---|---|
| `span` | The current span | When a new count held |
| `held: Vec<HeldBody>` | The held bodies, oldest to newest | Rules 1 to 4; which box stays in bed |
| `unexplained` | Bodies in bed that are nobody | Rule 6 |
| `bed_phase: BedPhase` | `Adopting` until the first climb in, then `Tracking` | Rule 6: adopt or not |
| `last_seen_in_bed`, `last_seen_on_floor` | The bodies of the last sample with someone in each zone | Which boxes stay held; the box of an unseen subject |
| `session` | The session of the last sample with bodies per zone | If it changes, the span starts over |

History (the "since when" and the last observation) is not in the machine: it
is `chronicle::Chronicle` (§6).

`apply` is deliberately not called `evolve`: `evolve` is the fold of the counts
in `conservation`, and a concept has a single name.

### Layer 1: the bed posture (`belief/bed_posture/`)

Whether the resident they left lying is still lying, sat up or is getting
out (R18). It exists only while layer 0 believes the resident alone in bed,
only when the policy follows it (`Policy.bed_posture`), and it only reports
leaving lying: entering the bed, nothing.

```text
             layer 0 ── InBed::{Seen, Held} or nothing ──┐   (Belief wires it)
                                                          ▼
refine::sighting ──[Sighting]──► rules::decide(l1(t−1), l0(t), sighting) ──► Stabilizer ──► Concluded
second level,  what layer 1    a pure decider: the golden table (5 × 8)     each state      state, seen
pure           perceives                                                    its own time
```

The row is `l1(t−1) : l0(t) + seen(t) : l1(t)`: layer 0 is an input of
layer 1, never the other way round.

- **What layer 1 perceives** (`refine::sighting`, pure, a `Sighting`), in two
  steps. First the **body's places** (`BodyPlaces`: where the head is, where
  the feet are as a set, the disparity), the trama's `hfd`. Then
  `BodyPlaces::sighting`, a decision list in this order: a foot past the bed's
  sides, the head past them, the head away from the headboard, and, only with
  the head at the headboard, the disparity (lying or upright). A kept pose
  places nothing: `unknown`. It does not read layer 0. The golden trama
  `contract/bed-posture-frame.txt` crosses every place with every state.
- **The rules** (`rules::decide`): without the resident alone in bed (layer
  0) there is no layer 1; it is born in bed; held in bed (layer 0) or lying
  at the headboard is lying; in bed, only lying moves it; once lying, what is
  seen now rules, symmetrically (sitting up, getting out, back to lying);
  unknown holds. The published `seen` composes both halves: `held` when
  layer 0 holds the resident.
- **The machine** (`Machine`, P1): the stable state and the candidate, with
  the layer-0 `Stabilizer`; unknown lets the candidate keep holding. It forgets
  when layer 0 no longer has the resident alone in bed.
- **Where the head is** (`body::HeadPlace`) is the fact that filters: the
  headboard region and, away from it, the bed's u/v (`below_headboard`,
  `at_side`). It is computed when the pose arrives
  (`attention::head_place`); the depth look and layer 1 read it.

`chronicle` dates the state (`bed_posture` since) and `project` publishes
`subject.bed_posture { state, seen, since }`, additively in v4. Each change
of the stable state while it is followed is a `bed_posture` event in
`events.v4`, also additive (the readers are ours, 2026-10-31): `{ at, cell,
before { state, since, lasted_secs }, seen, after }`, last in the turn. The
cell is a row of its golden table, `before | seen | after`. Starting or
stopping to follow is not a row and publishes nothing: getting into and out
of bed are layer 0's events.

### The occupancy of the parts (`belief/parts/`)

Where each part of the subject is against the two planes with their own
`u/v`, the bed and the headboard, and what is kept unseen. It is not the
posture: the posture relates the parts to each other, this relates them to
places. The sister of layer 0, with its shape and at its compass: once per
sample, with layer 0's dwell, and only while layer 0 has a subject (the only
occupant, anywhere).

```text
             layer 0 ── a subject or not ──┐   (Belief wires it)
                                           ▼
refine::parts ──[ByPart<Look>]──► rules::decide(places(t−1), subject, seen) ──► Stabilizer ──► Concluded
second level,  what the sample   a pure decider, position by position:        the places,    seen, places
pure           saw of each part  the golden table (5 × 5)                      without held   with held
```

The row is `l0[t] + places[t−1] : seen[t] : places[t]`.

- **What the sample saw** (`refine::parts`, pure, a `ByPart<Look>`): two
  bits per part, some point inside the region and some outside; both, split.
  Against the bed `cfhsaip` (the person's box centre, the face, the head, the
  shoulders, the arms, the groin, the legs); against the headboard `fhs`; and
  three whole-box flags `BFK` (the person's box in the bed, the face's in the
  bed, the face's in the headboard). A part counts only from a sense that
  looked in this sample (`-` otherwise): the box from the main detector, the
  faces from theirs, the rest from a fresh pose.
- **The rules** (`rules::decide`): without a subject there are no places and
  the memory is lost; a part seen takes where it was seen; a part not seen
  keeps where it was last seen, **held**; never seen, it is unknown.
- **The machine** (`Machine`, P1): the retention (where each part was last
  seen) and the places without the held marks, stabilized with layer 0's
  dwell. Held is not stabilized: it says where the datum of this sample comes
  from, like a provenance.

It is published as layer 0 is, being its complement: the scene carries
`subject.parts { state, seen, since, pending { places, duration }, last_cell }`
and `events.v4` a `parts` event per change of the stable places, `{ at, cell,
before { places, since, lasted_secs }, seen { parts, span }, after }`, last in
the turn so that layer 0's events stay together. The cell is
`before | seen | after`. `chronicle` folds the last change (since when, the
last cell) and forgets it without places. `project/parts.rs` writes the
notation, the same for every version (ADR 2026-10-14).

**The reading** (`belief/parts/reading.rs`) is what we believe from the
places: the semantic sub-state of layer 0 (`out_of_bed`, `on_the_edge`,
`lying`, `trying_to_leave`, `sitting_up`, `sitting_in_bed`, `undecided`). A
pure decision list over six facts of the places now (seen or held): the
groin and the box in the bed, the headboard (unseen, free, partly, lying),
something out (the arms aside) and the whole boxes in. Its golden table is
`contract/parts-reading-table.txt`. It is stabilized on its own, the urgent
ones (`on_the_edge`, `trying_to_leave`) with half of layer 0's dwell, and
published as `subject.parts.reading` with `reading_since`. Where two
situations put the parts in the same places (lying and sitting up against the
headboard) it does not choose: that is the depth's work, in layer 1.

## 6. Projections and observers

The core is Markov: it keeps only what it needs to decide what comes next
(P1). Everything else is a **fold** over a sequence, in the domain (P4):

- **`project`**: translates the belief and the chronicle into the contract.
  It is the only module that speaks `contract`, besides `cycle`'s `Output`;
  it does not decide and is not read back.
- **`chronicle::Chronicle`**: since when the room's name, the bed's kind and
  the second level's verdict (R16) hold, and the last turn that observed bodies per zone (the `observed_at` of
  a turn without the detector). It folds the domain (`RoomName`, `BedKind`,
  the transition's counts), not the wire: the bed goes to unknown without
  layer 0 changing, and `project` translates the fold. It lives **outside the
  engine**: `Monitor` keeps it, the engine does not see it, and `project`
  reads it already folded (`Chronicled`), without mutating it. A state change
  is both a new "since when" and a derived event (`room_changed`,
  `bed_changed`): both come out of the same fold. The previous scene
  (`Monitor.last`) stays only for republishing.
- **Outside the core**: dwell analysis, episodes and recordings are folds over
  the published contract, in other repositories (`calipso-shark`).

## 7. Glossary

Identifiers and prose are in English; the ubiquitous language rules (rule 20).
One concept, one name.

| Term | In the code | What it is |
|---|---|---|
| sample | `time::SampleRef` | An instant of the source: session and source time |
| session | `SampleRef::session` | A continuous stretch of the source; time is not compared across sessions |
| turn | `Turn` | The core's work on one sample |
| loop | `cycle`, `Monitor` | The sequence of turns; `Monitor` composes engine, chronicle and last scene |
| engine | `Engine` | What decides: policy, map, belief and sensor; it does not remember what was published |
| conclusion | `conclusion::Conclusion` | What the engine concludes from a closed turn, before the chronicle and the contract: an `Observation` or what to republish |
| observation | `conclusion::Observation` | What a turn observed and believed, together: the input of the chronicle and the projection |
| look, probe | `look`, `Probe` | What the core asks the shell to look at |
| focus | `FocusPolicy`, `SiteMap::focus`, `attention::primary_region` | Where the main detector looks while the resident is alone in bed: the bed up close |
| room geometry | `room`, `Course::room` | The only owner of the questions of space: where a body is and whether it is near the bed (layer 0); where the head, a part, a box or a foot is against the bed and its headboard (layer 1 and the second level). The turn builds it once and every layer asks it; it answers with named facts, remembers nothing and counts nothing. The crops are the map's. `check.sh` keeps the bed's plane out of the layers (white book, volume VII, ch. 01) |
| head area | `room::HeadArea` | The room's answer to where a point of the head falls: the headboard, the bed, past its sides or elsewhere. `refine::head` turns it into the published fact `HeadPlaceState`, which also says when no head was seen; the room does not depend on the layers above it |
| geometry (of the frame) | `map::Geometry` | Whether the incoming frame has the resolution the bed was delimited for (`Valid`, `Invalid`): published in `scene.v4` as `map.geometry`. Not the room geometry: the piece that answers the questions of space is `room` |
| frontal | `room::Frontal` | The room geometry of a camera that sees the bed along the image: in bed if the box's centre is in the bed, near the bed if the box reaches the bed's bounding box grown (in the bed frame, the bed grown in its own coordinates); the head, the parts and the feet in the bed's `u/v`. The engine's answers as they were (volume VII, ch. 02) |
| two planes | `room::TwoPlanes`, `TwoPlanesPolicy` | The room geometry of a camera that sees the bed on the diagonal, chosen by the profile: a body is in bed when enough of its box falls over the mattress and its support rests close to the bed's footprint on the floor; near the bed, by the support's distance in metres. The other questions of space as frontal (white book, volume VII, ch. 14) |
| support | `room::TwoPlanes`, `BodyShadow::support` | The centre of a box's lower edge: where a standing body rests on the floor, measured against the bed's footprint |
| doubtful body | `room::Room::doubtful` | A body the box alone cannot place: in two planes, one left on the floor by its support with enough of its box over the mattress. Its pose is asked before believing, and the room places it with it (white book, volume VII, ch. 15) |
| knee | `room::two_planes::knee_deg`, `TwoPlanesPolicy::knee_bent_deg`, `BodySeen::knee_deg` | The hip–knee–ankle angle of a pose, from the midpoints of both sides: bent, the pelvis rests on the mattress; straight, on the floor |
| a foot out | `Room::foot_out`, `Room::foot_beyond` | P6 for a foot, layer 1's legs out: frontal, past the mattress's sides; in two planes, out of the footprint on the floor past any edge, the foot end too, where a foot rests (white book, volume VII, ch. 04) |
| shadow | `ShadowPolicy`, `MapShadow` | Estimates of a geometry that does not decide yet, published next to what the engine decides, to choose with data; asked for by the profile, temporary (white book, volume VII, ch. 11) |
| bed footprint | `SiteMap::bed_floor` | Where the bed stands on the floor: the same rectangle as the mattress, drawn at floor level. Points on the floor (ankles, a box's lower edge) are measured against it, not against the mattress (volume VII, ch. 04) |
| bed size | `SiteMap::bed_size_m` | The bed's width and length in metres: it turns the bed's `u/v` into metres |
| near (in metres) | `ShadowPolicy::near_m`, `MapShadow::near` | Near the bed as a distance: the footprint grown by this many metres in its own coordinates, not the bed's bounding box grown by a fraction |
| widen | `attention::widen` | The focus did not confirm what is believed: the same turn looks at the whole frame before believing |
| sentinel, lens | `FocusPolicy::wide_every_secs`, `attention::Lens` | The whole frame at least this often; the lens remembers when it was last read |
| attention memory | `attention::Memory`, `Engine::attention_memory` | What attention remembers of what it already looked at, and nothing else: the lens, the reinforcement latch and the last calibration attempt. `cycle` notes each look in it; attention's policies read it |
| reinforcement latch | `attention::LatchState` | Someone was seen in bed by the main detector, so losing them there can be reinforced, once per loss. Part of attention's memory, not of the belief |
| reading | `Reading` | What the shell returns for a probe |
| evidence | `evidence` | Which reading counts and how much it is worth; it never decides what is believed |
| box | `look::Detection` | What a detector returns: a rectangle, a score and a class. It is evidence of a body, not a body |
| body (in the room) | `evidence::Body`, `Bodies`, `HeldBody` | A person, as an admitted box shows them: what layer 0 counts per zone |
| count | `zone::Counts` | The bodies seen per zone: floor and bed |
| occupants | `belief::Occupants` | Who layer 0 believes is there: off the bed, seen in bed and held in bed. The book's `RBH` is `(total, bed + held, held)` |
| span | `Span` | From the last stable count until a new one holds |
| stabilize | `Stabilizer` | A value counts once it has held in source time |
| what we believed / what we saw | `Before` / `Seen` | The two input columns of the table |
| movement | `Movement` | What explains a change: `LeftBed`, `Held`, `HeldLeft`, `Reappeared`, `LayDown`, `Adopted`, `LeftRoom`, … |
| settle | `settle` | Which movements explain the change, in order |
| evolve | `evolve` | How a movement moves the counts |
| held | `held`, `HeldBody`, `InBed::Held` | Stopped being seen in bed without leaving: still there (rule 1) |
| side exit | `Movement::HeldLeft`, `Policy::held_left` | Rule 4 bis, a capability of the profile: with someone held in bed, a body that appears on the floor near the bed, not from the door, is the held one who got up by the side. Published as `left_bed` (white book, volume VII, ch. 05) |
| near first | `Policy::near_first` | Rule 5, refined, a capability of the profile: whoever appeared near the bed and then walked away while the count stabilized did not come from the door; they got up and are heading for it (white book, volume VII, ch. 17) |
| unexplained | `unexplained` | A body in bed that did not come from the floor (rule 6) |
| adopt | `BedPhase::Adopting`, `Movement::Adopted` | Before the first climb in, whatever appears in bed is adopted |
| fact | `Fact`, `Happened` | What happened, in the past tense: `Entered`, `Adopted`, `LayDown`, `HeldInBed`, …; `Happened` carries it with the occupants it leaves |
| transition | `Transition` | A table row as a value: the phase it was decided with, what we believed, what we saw (and from which sample), what we believe and the facts |
| tally | `Tally` | What layer 0 believes at an instant: occupants per zone and unexplained bodies; before, after and on each fact |
| machine | `occupancy::Machine`, `Machine::step` | Layer 0: receives a sample and returns the transition, if any |
| mode | `Room` | The name of the layer-0 state |
| subject | `Subject` | The only occupant, if there is exactly one |
| the subject's body, pose | `body::Pose`, `BodyPoint`, `Joint`, `SubjectBody` | The subject's COCO-17 keypoints and their relative depth: what level 2 looks at |
| enrichment | `Enrichment`, `Belief::observe_body` | What a turn did with the subject's body: a fresh pose (and depth), kept, failed or nothing |
| provenance | `provenance::Provenance` | Where a claim comes from: current, retained or stable (rule 2) |
| published language | `contract::v4` (and `contract`) | What a version publishes; it re-exports the domain vocabulary |
| second level | `refine` | How the resident is: lying, posture |
| body group | `refine::BodyGroup`, `BodyGroups` | Head, shoulders or pelvis: the median depth of its seen keypoints and how many it saw (R16) |
| disparity | `Posture::disparity`, `Disparities` | `(upper − pelvis) / mean`: head and shoulders together against the pelvis decide; the feet do not vote (R16) |
| bed point | `BedPoint`, `BedSite`, `Posture::bed_points` | A fixed site of the bed (today its centre, u/v 0.5) whose depth is read in the same look as the body, with the resident in bed: covered, the blanket over the hips. `upper_bed_centre` is the baseline next to the pelvis' disparity; context, not a vote (R17, 1707) |
| head place | `body::HeadPlace`, `Posture::head_place` | Where the head is (at the headboard, below it in bed, past the bed's sides, elsewhere, unknown) and what said so: a fact of its own, as the occupancy is where the body is. The bed's centre is the hips only with the head at the headboard; readers read the fact, not how it was concluded (R17, 1707) |
| head (of the second level) | `refine::Head` | An independent opinion on how the resident is: today `disparity`; the bed plane, the head's place in the bed and a VLM will follow |
| arbiter, verdict | `refine::verdict`, `Verdict`, `VerdictState` | The table from the heads' opinions to one verdict, with the heads that support it and since when; only in bed for now (`lying_in_bed`, `sitting_up_in_bed`, `in_bed`) |
| projection | `project` | How the belief looks from outside |
| fold | `chronicle::Chronicle` | A value computed over a sequence: of the domain (the room's name, the bed's kind, a transition's counts) |
| chronicle | `chronicle::Chronicle`, `Entry`, `Chronicled` | The memory outside the engine, in the domain: since when the room's name and the bed's kind hold, what changed, and the last transition's counts. `project` translates it |
| since when | `chronicle::Since`, `Stamp` | A state and the sample it holds since; dated on a turn, with the previous one if it changed |
| core / shell | `scene-core` / `detector` | Without IO / with IO |
| table | `machine/table.rs`, `contract/layer-0-table.txt` | Every layer-0 combination with its result |
| parts, places | `belief::parts`, `ByPart`, `Look`, `Place`, `Parts` | Where each part of the subject is against the bed and the headboard (`cfhsaip · fhs · BFK`), two bits each (inside, outside), and what is held: not seen in this sample, kept where it was last seen. The sister of layer 0, at its compass |
| reading (of the parts) | `belief::parts::PartsReading`, `reading::Facts` | What we believe from the parts' places, the second sentence: a decision list over six facts with its golden table; it leaves to the depth what occupancy cannot tell |
| layer 1, bed posture | `belief::bed_posture`, `BedPostureState` | Whether the resident left lying is still lying: `in_bed`, `lying`, `sitting_up`, `getting_out`. The sister of layer 0, with its shape (R18) |
| verification, verifier | `verification`, `Probe::Verify`, `Monitor::verified` | Asking a slow verifier (a VLM over a crop) a closed question, outside the turn: the answer is evidence of the sample it looked at and never moves a count (book 4; white book, volume III) |
| doubtful digit, grey band | `DisparitySide::Doubtful`, `policy::Band`, `VerifyPolicy::doubtful_disparity` | A body place whose reading falls in the camera's grey band (`?` in the trama): layer 1 reads it as nothing new, and the verifier may fill it. The answer fills the digit of the looked sample; layer 1 runs its own, unchanged table on it (white book, volume III, ch. 10) |
| ticket | `look::Ticket`, `Target` | A request to the verifier: its id, the sample it looks at and what it asks about, by identity (a held body by its `last_seen`). A late answer applies only if that same thing still holds |
| held check | `HeldCheck`, `HeldCheckState` | The provenance of a held body: unverified, confirmed or not confirmed, by whom and looking at which sample. Not confirmed does not release (frontier 3) |
| body places | `refine::BodyPlaces`, `FeetPlace` | Where each part layer 1 reads is in a sample: the head (`HeadPlaceState`), the feet as a set (unseen, seen, past the bed's sides: one out is out) and the disparity (`PostureState`). The trama's `hfd`; a kept pose places nothing |
| sighting, seen (layer 1) | `bed_posture::Sighting`, `refine::sighting`; `BedPostureSeen` | What layer 1 perceives by itself in a sample (`headboard_lying`, `headboard_upright`, `legs_out`, `head_side`, `head_away`, `unknown`); published as `seen`, which says `held` when layer 0 holds the resident |
| decide | `bed_posture::rules::decide` | Layer 1's pure decider, the row `l1(t−1) : l0(t) + seen(t) : l1(t)`; the golden table `contract/bed-posture-table.txt` |

## 8. Module map

Each module is a step of the loop or a piece without domain. The last column
matters as much as the second.

| Module | Responsibility | Never |
|---|---|---|
| `lib.rs` | The public API and rule 21's `deny` | exposes how the core believes |
| `cycle` (+ `monitor`, `engine`, `sequence`, `waiting`) | The loop protocol (`cycle`), what one camera is made of and what comes out of a turn (`monitor`), what the engine changes when the turn asks it (`engine`), the order of a turn and its ledger (`sequence`), each probe with its continuation (`waiting`) | decides anything |
| `look` | The protocol's vocabulary: probes, readings, what was asked (`LookKind`) and how it failed | contains pixels |
| `evidence` (+ `bodies`, `faces`, `pose`, `depth`) | Admits readings, one file per sense: one body per person, its zone | decides what is believed or looked at |
| `belief` | The belief: joins layer 0, the subject and layer 1 | sees a raw reading |
| `belief/occupancy` | Layer 0: the occupancy machine | knows the second level or the contract |
| `verification` | The verification plane: what the slow verifier was asked (one ticket in flight), the answers owed, and the held body's provenance (`HeldCheck`). Answers arrive between turns (`Monitor::verified`) and are settled at the next sample's turn | moves a count, or is read by layer 0 or layer 1 |
| `belief/bed_posture` | Layer 1: the bed posture machine, its rules (a golden table) and its vocabulary | changes a count, or reads a raw reading |
| `belief/stabilizer` | Source-time stabilizing, for both layers | knows what it counts |
| `belief/subject` | The subject's enrichment: pose, depth, reference | decides occupancy |
| `belief/parts` | The occupancy of the subject's parts: where each part is and what is held | changes a count or reads the posture |
| `refine` | The second level: where the head is (`head`, from the faces and the pose, asking the room), lying, posture by body groups, the arbiter's verdict (a golden table) and what layer 1 saw (`seen`) | writes the belief or changes a count |
| `attention` (+ `primary`, `faces`, `body`, `depth`, `verify`, `memory`, `ledger`) | What to look at, where and why not, one file per question; its ledger (`Attention`, `Look`, `SkipReason`); its memory (`Memory`: the lens, the reinforcement latch and the last calibration attempt) | mutates the belief |
| `body` | Body vocabulary: the COCO-17 pose and its depth, the bed's sites and where the head is (`HeadPlace`, concluded by `refine::head`) | interprets the pose (that is `refine`) |
| `provenance` | Where a claim comes from (rule 2) | is inferred outside the belief |
| `conclusion` | The seam between deciding and publishing: what the engine delivers when the turn closes | depends on `project` or `chronicle` |
| `chronicle` | The memory of what was concluded, in the domain: folds the turns | is read by the engine to decide, or imports the contract |
| `project` (+ `v4`) | Translates the domain into the contract, one version per module, from the same conclusion and the same chronicle; the shared part is projected once | decides, mutates the chronicle or is read back |
| `contract` | The published language, per version. `common`: what no version changes (the envelope without its `SchemaId`, the bed, the subject, the evidence, the causes) and the re-export of the domain vocabulary. `v4` defines its own `SchemaId` and schema; a next version would not depend on it. `contract` re-exports the published version | changes without a major version and an ADR, or is imported by the domain |
| `policy` | The policies that come from the profiles | is read from a file |
| `map` | The site's static world | is inferred from the subject |
| `room` (+ `frontal`, `two_planes`, `shadow`) | The questions of space, with named facts: the piece the profile chose (`frontal`, `two_planes`) and the shadow of a geometry that does not decide yet (`shadow`) | remembers, counts, or hands a plane to a layer |
| `sensor` | The reception state the shell reports | is inferred |
| `time`, `zone` | Shared vocabulary: the sample and its source time; the zones and the count per zone. `contract` re-exports them | knows the published format |
| `geom`, `estimation` | Maths without domain | knows a rule |

## 9. How a module is written

Every file opens with a `//!` that answers, in this order:

1. **What it is**, in one sentence, and which step of the loop it is.
2. **What it never does**, when there is a concrete temptation.
3. **The invariants** it holds, with their rule.
4. **See also**: the book or the spec that explains the why.

Names: they say what something is, not how it is computed; the module path
gives the context (`occupancy::Machine`, not `OccupancyMachine`); one concept,
one name across the crate; a `bool` that is a phase is an `enum`. Short names
are for local scope.

## 10. How to verify

- Three levels (rule 17): the crate that changed while working;
  `scripts/quick.sh` on every commit (format, clippy and the tests of the
  crates that changed and of those that depend on them); `scripts/check.sh`
  once per version, before a branch goes into `main` (format, clippy, core
  dependencies, every test and licenses).
- The golden tables (layer 0, the parts and their reading, layer 1, the
  second level's arbiter, V) share one net, `golden::assert_table`: if a rule
  changed on purpose, the test prints the rows that changed and the whole new
  table to copy into its file under `contract/`, and the diff goes into the
  commit.
- The golden scenes and their replay through the detector: the contract gives the same.
- A refactor does not change a table cell, a published event or a `.feature`.

## 11. Design debt

What we know must move, with the round that moves it. A row is deleted when its
round is integrated.

| Debt | Why | Round |
|---|---|---|
| The burst's phases (collect, keep, dispatch in `verification.rs`) are imperative. What an answer or an expiry does is already a decider with its golden table (`verification::decide`) | The burst (white book III, ch. 12) adds a phase to V; its rows belong in a table, each with its scenario and mutation, not in loose logic | V's phases as a pure machine, with the burst |
| The domain vocabulary carries the serde and schemars derives | Like `time` and `zone`: the value knows how to serialize and each version decides what it publishes. A change meant for one version in a shared type changes every version: `contract_schema` catches it | accepted |
| `subject.depth.meters` and `calibration.reference_m` are named in metres but carry the depth model's relative scale | Their descriptions say so; renaming them changes the wire, so it waits for the next contract version (C2), together with the positional `rbh` and `fv` | next contract version |
| The scene's `last_event_seq` is written by the NATS actor (the last event delivered), not where the stream is numbered | The transport writes into the published body: the in-memory sink and NATS agree only while no event is dropped. Moving it changes what the field means to its consumers (C1) | next contract version |
| `Input::Sensor` carries a `SampleRef`, and the shell makes one up from the last sample (`Driver::sensor_at`) | The turn's `at` is published as the envelope's `observed_at`, which is not optional: a reception change with no sample needs it to be optional | next contract version |
| The second level's verdict (R16: `lying_in_bed`, `sitting_up_in_bed`, `in_bed`, a reading per pose) and layer 1's bed posture (R18: a belief with memory) both say how the resident is in bed | The verdict is published (C1) and its consumers read it; layer 1 says more and better (only at the headboard, held is lying, leaving lying). Two answers to one question confuse a consumer | next contract version: retire `subject.verdict` once the consumers read `bed_posture` |
| The contract re-exports domain types instead of owning wire types | Consumers depend on `scene-core` to read the contract. A separate wire crate (like the kernel's `uapi` headers) would let consumers depend only on the published language | next |

## Sources

- Book 1, ch. 03 (a core without IO) and ch. 06 (events and projections). Book 3
  (layer 0 and the table). Book 4 (verifying the held occupant). In
  `calipso-docs`.
- J. Chassaing, *Functional Event Sourcing Decider* (2021). M. Fowler, *Event
  Sourcing* (2005). E. Evans, *Domain-Driven Design* (2003): the ubiquitous
  language. G. Booch, *Object-Oriented Analysis and Design* (2007): abstraction
  and modularity. *Rust API Guidelines*. The Linux kernel,
  `Documentation/trace/tracepoints.rst`. Diátaxis: reference and explanation,
  kept apart.
