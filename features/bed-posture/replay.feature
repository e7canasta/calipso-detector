# language: en
Feature: The bed posture replays
  As care staff
  I want the same night to give the same bed posture
  so that a night can be reviewed without a camera

  # Layer 1 (R18, 1806), the sister of the occupancy: its row is
  # l1(t-1) : l0(t) + seen(t) : l1(t), with its golden table in
  # contract/bed-posture-table.txt. It reads layer 0 and never breaks it.

  Background:
    Given a room with the bed delimited
    And the bed's headboard delimited
    And stabilization requires 2 seconds of source time
    And samples arrive every 1 second of source time
    And there is a pose model
    And there is a depth model
    And the motion threshold is 0.1 of the subject's diagonal
    And the bed posture is followed

  Rule: The same night gives the same bed posture

    Scenario: A night replays without a camera and gives the same
      Given the bed posture "sitting_up" must hold 2 seconds of source time
      And the resident came in alone and lay down in samples 1 to 10
      When the camera sees this sequence:
        | samples | on floor | where | in bed | subject moves | pose        | occupants | bed            |
        | 11      | 0        |       | 1      | yes           | lying       | 1         | occupied, seen |
        | 12-14   | 0        |       | 1      |               |             | 1         | occupied, seen |
        | 15      | 0        |       | 1      | yes           | sitting up  | 1         | occupied, seen |
        | 16-18   | 0        |       | 1      |               |             | 1         | occupied, seen |
        | 19      | 0        |       | 1      | yes           | leaning out | 1         | occupied, seen |
        | 20      | 0        |       | 1      | yes           | lying       | 1         | occupied, seen |
        | 21-30   | 0        |       | 0      |               |             | 1         | occupied, held |
      Then the same night, replayed with the same readings, gives the same scenes and events
      And in sample 18 the bed posture is "sitting_up"
      And in sample 30 the bed posture is "lying", seen "held"
