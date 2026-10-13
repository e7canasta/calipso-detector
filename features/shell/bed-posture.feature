# language: en
@shell
Feature: The bed posture profile
  As whoever installs a camera
  I want to switch the bed posture on and tune it in the camera's profile
  so as to make it as sensitive as that camera's position and distance need

  # Layer 1 (R18, 1804): without the section nothing follows it. It needs
  # the headboard delimited (where the head is) and a pose model.

  Scenario: Without "[bed_posture]" nothing follows the bed posture
    Given a profile without "[bed_posture]"
    Then the core policy does not follow the bed posture
    And the profile hash is the baseline one

  Scenario: The section switches it on, each state with its own time
    Given a profile with "[bed_posture]" holding lying 2.5, sitting up 4 and getting out 0.5 seconds
    Then the core policy follows the bed posture holding lying 2.5, sitting up 4 and getting out 0.5 seconds

  Scenario: Without the times, the initial ones: getting out is the shortest
    Given a profile with "[bed_posture]" and no times
    Then the core policy follows the bed posture holding lying 2, sitting up 3 and getting out 1 seconds

  Scenario: A negative time is rejected
    Given a profile with "[bed_posture]" holding lying 2, sitting up -1 and getting out 1 seconds
    Then the profile is rejected with a reason that names "bed_posture.dwell_sitting_up_secs"

  Scenario: The bed posture needs the headboard delimited
    Given a profile with "[bed_posture]" but no headboard
    Then the profile is rejected with a reason that names "map.headboard"

  Scenario: The bed posture needs a pose model
    Given a profile with "[bed_posture]" but no pose model
    Then the profile is rejected with a reason that names "senses.pose"
