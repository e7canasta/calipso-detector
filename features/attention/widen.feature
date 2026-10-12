# language: en
Feature: The focus only confirms; otherwise the turn widens
  As care staff
  I want the engine, if it looks at the bed up close and does not see what it expected, to look at the whole room before deciding anything
  so that looking up close never makes me believe someone is still in bed who got up and left

  # The focus confirms only what is believed: alone in bed and seen, it sees
  # one in bed; held, it sees nobody or the one who reappears. Anything else
  # (someone on the floor, two in bed, nobody where one was seen) and the same
  # turn looks at the whole frame before believing. Layer 0 never receives a
  # reading from the focus that did not confirm.

  Background:
    Given a room with the bed delimited
    And stabilization requires 2 seconds of source time
    And samples arrive every 1 second of source time
    And the main detector focuses on the bed grown by 0.25 and looks at the whole frame every 30 seconds of source time

  Scenario: Gets up and the narrow focus misses them by the bed: widening sees them leave
    # With a pad of 0.1 the focus ends at x=540: a body "near bed" (centroid
    # x=555) falls outside it. The sample that shows them by the bed is the one
    # that explains the exit (rule 6); without it, the door would be a newcomer.
    Given the main detector focuses on the bed grown by 0.1 and looks at the whole frame every 30 seconds of source time
    And the resident came in alone and lay down in samples 1 to 10
    When the camera sees this sequence:
      | samples | on floor | where    | in bed | occupants | bed            |
      | 11-15   | 0        |          | 1      | 1         | occupied, seen |
      | 16-18   | 1        | near bed | 0      | 1         | empty          |
      | 19-25   | 1        | door     | 0      | 1         | empty          |
    Then in sample 16 the main detector looked at the focus and then at the whole frame
    And an exit from the bed was observed
    And nobody is held in bed

  Scenario: Sits on the edge: the focus sees the floor and widens
    Given the resident came in alone and lay down in samples 1 to 10
    When the camera sees this sequence:
      | samples | on floor | where    | in bed | occupants | bed   |
      | 11-20   | 1        | near bed | 0      | 1         | empty |
    Then in sample 11 the main detector looked at the focus and then at the whole frame
    And an exit from the bed was observed

  Scenario: Someone comes in near the bed: it widens and the room is accompanied
    Given the resident came in alone and lay down in samples 1 to 10
    When the camera sees this sequence:
      | samples | on floor | where    | in bed | occupants | bed            |
      | 11-20   | 1        | near bed | 1      | 2         | occupied, seen |
    Then in sample 11 the main detector looked at the focus and then at the whole frame
    And the room is "Several"
    And in sample 20 the main detector looked at the whole frame

  Scenario: Covers up: it widens, the whole frame does not see them either, held as today
    Given the resident came in alone and lay down in samples 1 to 10
    When the camera sees this sequence:
      | samples | on floor | where | in bed | occupants | bed            |
      | 11-20   | 0        |       | 0      | 1         | occupied, held |
    Then in sample 11 the main detector looked at the focus and then at the whole frame
    And the bed occupant is held since sample 10
    And no exit from the bed was observed

  Scenario: Covers up with reinforcement: reinforcement comes after widening
    Given there is a reinforcement detector
    And the resident came in alone and lay down in samples 1 to 10
    When the camera sees this sequence:
      | samples | on floor | where | in bed | reinforcement in bed | occupants | bed            |
      | 11      | 0        |       | 0      | 1                    | 1         | occupied, seen |
    Then in sample 11 the looks were "detect, detect, reinforce"
    And in sample 11 reinforcement was requested over the grown bed

  Scenario: Held and unseen: the focus confirms and does not widen
    Given the resident came in alone and lay down in samples 1 to 10
    When the camera sees this sequence:
      | samples | on floor | where | in bed | occupants | bed            |
      | 11-20   | 0        |       | 0      | 1         | occupied, held |
    Then in sample 20 the looks were "detect"
    And in sample 20 the main detector looked at the bed grown by 0.25

  Scenario: Held and reappearing in bed: the focus confirms and does not widen
    Given the resident came in alone and lay down in samples 1 to 10
    When the camera sees this sequence:
      | samples | on floor | where | in bed | occupants | bed            |
      | 11-20   | 0        |       | 0      | 1         | occupied, held |
      | 21-25   | 0        |       | 1      | 1         | occupied, seen |
    Then in sample 21 the looks were "detect"
    And the bed occupant reappearing was observed

  Scenario: If the whole-frame look fails, the scene is declared and nobody is held because of it
    Given the resident came in alone and lay down in samples 1 to 10
    And the whole-frame look fails in samples 16 to 18
    When the camera sees this sequence:
      | samples | on floor | where | in bed | occupants | bed            |
      | 11-15   | 0        |       | 1      | 1         | occupied, seen |
      | 16-18   | 1        | door  | 0      | 1         | occupied, seen |
    Then the scene declares no new observation since sample 15
    And nobody is held in bed
