# language: en
Feature: Contract v5: the same turn, cut at the five changes
  As someone reading a night on the next contract
  I want the scene of v5 to state the mode, the depths and the transitions as the contract names them
  so that a consumer reads v5 with the names the ADR gives it

  Background:
    Given a room with the bed delimited
    And stabilization requires 2 seconds of source time
    And samples arrive every 1 second of source time

  Scenario: The v5 scene states the mode, the phase and the last cell
    Given the resident came in alone and lay down in samples 1 to 10
    Then the v5 scene of sample 10 says mode "alone_in_bed_seen", phase "running" and last cell "100:01:110"


  Scenario: Change 3: the v5 scene has no subject verdict
    Given there is a pose model
    And there is a depth model
    And the motion threshold is 0.1 of the subject's diagonal
    Given the resident came in alone and lay down in samples 1 to 10
    When the camera sees this sequence:
      | samples | on floor | where | in bed | subject moves | pose  | occupants | bed            |
      | 11      | 0        |       | 1      | yes           | lying | 1         | occupied, seen |
    Then the v5 scene of sample 11 has no subject verdict

  Scenario: Change 4: the depths carry their relative names
    Given there is a pose model
    And there is a depth model
    And the motion threshold is 0.1 of the subject's diagonal
    Given the resident came in alone and lay down in samples 1 to 10
    When the camera sees this sequence:
      | samples | on floor | where | in bed | subject moves | pose  | occupants | bed            |
      | 11      | 0        |       | 1      | yes           | lying | 1         | occupied, seen |
    Then the v5 subject depth of sample 11 is named "relative" with 2.14

  Scenario: Change 5: a v5 transition names rbh and fv, with the numbers of its occupancy
    Given the resident came in alone and lay down in samples 1 to 10
    When the camera sees this sequence:
      | samples | on floor | where    | in bed | occupants | bed            |
      | 11      | 1        | door     | 1      | 1         | occupied, seen |
      | 12-13   | 1        | near bed | 0      | 1         | occupied, seen |
      | 14-15   | 1        | near bed | 0      | 2         | occupied, held |
    Then the v5 transition "110:10:211" names rbh as room, bed and held with 2, 1, 1
    And the v5 transition "110:10:211" names fv as floor and bed with 1, 0

  Scenario: Change 4: the bed's depth reference is named reference
    Given there is a depth model
    And the bed is calibrated with a retry of 10 seconds of source time
    And the bed has no depth reference
    When the camera sees this sequence:
      | samples | on floor | where | in bed | occupants | bed   |
      | 1-20    | 0        |       | 0      | 0         | empty |
    Then the bed was calibrated in sample 1
    And the v5 bed calibration of sample 5 is named "reference" with 2.0

  Scenario: Each fact states the occupancy it leaves
    Given the resident came in alone and lay down in samples 1 to 10
    When the camera sees this sequence:
      | samples | on floor | where    | in bed | occupants | bed            |
      | 11      | 1        | door     | 1      | 1         | occupied, seen |
      | 12-13   | 1        | near bed | 0      | 1         | occupied, seen |
      | 14-15   | 1        | near bed | 0      | 2         | occupied, held |
    Then layer 0 published transition "110:10:211" with these facts:
      | fact        | leaves | mode              |
      | held_in_bed | 111    | alone_in_bed_held |
      | entered     | 211    | accompanied       |

  Scenario: The scene states the mode and links the last cell
    Given the resident came in alone and lay down in samples 1 to 10
    Then the v5 scene of sample 10 says mode "alone_in_bed_seen", phase "running" and last cell "100:01:110"

  Scenario: room_changed goes by mode
    Given the resident came in alone and lay down in samples 1 to 10
    Then events.v5 published "room_changed" from "alone_out" to "alone_in_bed_seen"

  Scenario: A two-digit count is written as a tuple
    # The cell's format, not an engine behaviour: the scenes' world
    # models at most two people on the floor.
    Given a transition from "12,0,0" seeing "13,0" to "13,0,0"
    Then its cell is "(12,0,0):(13,0):(13,0,0)"
    Given a transition from "9,1,1" seeing "10,0" to "10,1,1"
    Then its cell is "911:(10,0):(10,1,1)"

  Scenario: Every transition agrees with its facts and its scene
    Given the staff put the resident to bed and left in samples 1 to 25
    Then on every turn, each transition agrees with its facts and its scene
