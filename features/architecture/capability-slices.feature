# language: en
@architecture
Feature: A disabled capability does not exist for the deciders
  As the engine's maintainer
  I want each decider to receive only its own policy slice, resolved when the monitor is built
  so as to change one capability without the occupancy count reading it
  (ADR 2026-11-02, politica-en-rebanadas; R19 UC-A2)

  # The third scenario of the first draft (the occupancy names no other
  # policy) is a rule of form: scripts/check.sh, fixture occupancy-reads-verify.

  Scenario: Without a verification section, the occupancy does not depend on any verifier answer
    Given a profile without "[verify]"
    And the bed holds a resident who lay down in samples 1 to 10 and is not seen for 3 samples with a dwell of 2 seconds
    When a verifier answers "yes" to a question about that body
    Then the room occupants are the same as without the answer
    And the published bed state is held

  Scenario: A bed posture section switches on its decider once, at build time
    Given a profile with "[bed_posture]"
    When the monitor is built
    Then the bed posture decider exists
    And a profile without "[bed_posture]" builds no bed posture decider

