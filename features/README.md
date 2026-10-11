# The executable specification

Every behaviour of the engine is a Gherkin scenario here, written in the
glossary's language ([`DESIGN.md` §7](../crates/scene-core/DESIGN.md)). The BDD
harnesses run every `.feature` against the real loop with scripted senses: no
camera, no images, no models. A scenario that does not run is not specified
(rule 15).

## Layout

One directory per capability of the engine, one feature per behaviour:

| Directory | What it specifies | Where it lives |
| --- | --- | --- |
| [`occupancy/`](occupancy) | Layer 0: how many are in the room and in bed, what the bed retains, the staff who come and go, stabilization by source time, what the scene declares when it did not observe | `belief/occupancy` |
| [`attention/`](attention) | What the core decides to look at, where, in what order and why not: reinforcement, faces, pose, depth, when to look again, calibrating the empty bed | `attention`, `belief/subject` |
| [`posture/`](posture) | The second level: the depth of each keypoint, truly lying, posture by disparity | `refine` |
| [`contract/`](contract) | What `calipso.scene` and `calipso.events` publish: the scene, the mode and the layer-0 transition | `project`, `contract` |
| [`shell/`](shell) | The shell's side: each capability answered by a real model adapter, the profile, distilling a night | `sense`, `detector`, `golden` |

The tables of `occupancy/` and `attention/` are the same language as the
[golden scenes](../golden/README.md): a span per row, what is seen on the left,
what is believed on the right.

## Tags

| Tag | Meaning | Who runs it |
| --- | --- | --- |
| (none) | The core's policy | `cargo test -p scene-core --test bdd` |
| `@shell` | The shell's capability: adapters, profile, publishing | `cargo test -p detector --test bdd` |
| `@wip` | A specification in progress, still red; it does not block integration | `scripts/wip.sh` |

`scripts/check.sh` runs both harnesses; an undefined step or a skipped
scenario fails them.

## Writing a scenario

- Start from the user story in the feature's narrative (`As … I want … so …`);
  each accepted path of the use case is one scenario.
- Put it in the directory of the capability it specifies. A new capability
  gets a new feature, not a section in an existing one; a behaviour that only
  refines another goes in as a `Rule:` of that feature.
- Prepare the belief by driving turns (`Given the resident came in alone and
  lay down in samples 1 to 10`), never by writing it by hand.
- A feature that adds a sense to the loop (`attention/`, `posture/`) carries
  the replay scenario: the same night, replayed with the same readings, gives
  the same scenes and events. Layer 0 is replayed by the golden scenes.
- Run one scenario with `cargo test -p scene-core --test bdd -- --name "<scenario>"`.
