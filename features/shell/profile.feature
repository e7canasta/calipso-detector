# language: en
@shell
Feature: The profile decides which contract versions are published
  As whoever operates a camera
  I want to choose in the profile whether v3, v4 or both are published
  to migrate consumers one at a time without breaking any (D9-4)

  The shell publishes what the core produces, according to the profile. R13-4
  turns it green, in the `detector` harness.

  Scenario: With the v3 profile, v4 does not go out
    Given the profile publishes only contract "v3"
    When the resident enters alone in samples 1 to 5
    Then no "calipso.scene.v4" or "calipso.events.v4" message was published
    And scene.v3 and events.v3 are identical to those of a profile that also publishes v4

  Scenario: With the v3 and v4 profile both go out, each with its numbering
    Given the profile publishes contracts "v3" and "v4"
    When the resident enters alone in samples 1 to 5
    Then "calipso.scene.v3" and "calipso.scene.v4" scenes were published for the same samples
    And events.v4 has its own numbering and its own last_event_seq

  Scenario: Without the section, the profile publishes v3 and v4 (R13-5)
    Given the profile without the "[publish]" section
    When the resident enters alone in samples 1 to 5
    Then "calipso.scene.v3" and "calipso.scene.v4" scenes were published for the same samples
