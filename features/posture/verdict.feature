# language: en
Feature: The second level's verdict in bed
  As care staff
  I want a single verdict of how the resident is in bed, saying which heads support it and since when
  to trust "lying" or "sitting up" knowing what it rests on

  # R16, 1605. The camera looks down from above: in bed, the head and shoulders
  # farther than the pelvis can only be lying. For now the verdict is the bed
  # zone (layer 0) and the disparity head; off the bed nothing is ruled yet.
  # The arbiter is a table (contract/second-level-table.txt).

  Background:
    Given a room with the bed delimited
    And stabilization requires 2 seconds of source time
    And samples arrive every 1 second of source time
    And there is a pose model
    And there is a depth model
    And the motion threshold is 0.1 of the subject's diagonal

  Scenario: In bed, farther above than the pelvis: lying in bed
    Given the resident came in alone and lay down in samples 1 to 10
    When the camera sees this sequence:
      | samples | on floor | where | in bed | subject moves | pose  | occupants | bed            |
      | 11      | 0        |       | 1      | yes           | lying | 1         | occupied, seen |
    Then in sample 11 the verdict is "lying_in_bed", by "disparity"

  Scenario: In bed, closer above than the pelvis: sitting up in bed
    Given the resident came in alone and lay down in samples 1 to 10
    When the camera sees this sequence:
      | samples | on floor | where | in bed | subject moves | pose       | occupants | bed            |
      | 11      | 0        |       | 1      | yes           | sitting up | 1         | occupied, seen |
    Then in sample 11 the verdict is "sitting_up_in_bed", by "disparity"

  Scenario: The verdict says since when it holds
    Given the resident came in alone and lay down in samples 1 to 10
    When the camera sees this sequence:
      | samples | on floor | where | in bed | subject moves | pose       | occupants | bed            |
      | 11      | 0        |       | 1      | yes           | lying      | 1         | occupied, seen |
      | 12-14   | 0        |       | 1      |               |            | 1         | occupied, seen |
      | 15      | 0        |       | 1      | yes           | sitting up | 1         | occupied, seen |
      | 16-20   | 0        |       | 1      |               |            | 1         | occupied, seen |
    Then in sample 14 the verdict is "lying_in_bed" since sample 11
    And in sample 20 the verdict is "sitting_up_in_bed" since sample 15

  Scenario: Off the bed nothing is ruled yet
    Given the resident came in alone and lay down in samples 1 to 10
    When the camera sees this sequence:
      | samples | on floor | where    | in bed | subject moves | pose     | occupants | bed   |
      | 11-14   | 1        | near bed | 0      | yes           | standing | 1         | empty |
    Then in sample 14 the subject is on the floor
    And in sample 14 there is no verdict

  Scenario: Without depth there is no posture and so no verdict
    Given there is no depth model
    And the resident came in alone and lay down in samples 1 to 10
    When the camera sees this sequence:
      | samples | on floor | where | in bed | subject moves | pose  | occupants | bed            |
      | 11      | 0        |       | 1      | yes           | lying | 1         | occupied, seen |
    Then in sample 11 there is no verdict

  Scenario: The verdict changes no count
    Given the resident came in alone and lay down in samples 1 to 10
    When the camera sees this sequence:
      | samples | on floor | where | in bed | subject moves | pose       | occupants | bed            |
      | 11      | 0        |       | 1      | yes           | sitting up | 1         | occupied, seen |
      | 12-15   | 0        |       | 1      |               |            | 1         | occupied, seen |
    Then in sample 15 the verdict is "sitting_up_in_bed", by "disparity"
    And no exit from the bed was observed

  Scenario: A night replays without a camera and gives the same
    Given the resident came in alone and lay down in samples 1 to 10
    When the camera sees this sequence:
      | samples | on floor | where | in bed | subject moves | pose       | occupants | bed            |
      | 11      | 0        |       | 1      | yes           | lying      | 1         | occupied, seen |
      | 12-14   | 0        |       | 1      |               |            | 1         | occupied, seen |
      | 15      | 0        |       | 1      | yes           | sitting up | 1         | occupied, seen |
      | 16-20   | 0        |       | 1      |               |            | 1         | occupied, seen |
    Then the same night, replayed with the same readings, gives the same scenes and events
