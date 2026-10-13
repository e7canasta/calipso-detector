# language: en
Feature: The bed posture: the disparity, only at the headboard
  As care staff
  I want the depth to decide lying or sitting up only with the head at the headboard
  so that the geometry rules everywhere else and back at the headboard is lying again

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

  Rule: At the headboard, and only there, the disparity says lying or sitting up

    Scenario: Lying in sight at the headboard is lying
      Given the resident came in alone and lay down in samples 1 to 10
      When the camera sees this sequence:
        | samples | on floor | where | in bed | subject moves | pose  | occupants | bed            |
        | 11      | 0        |       | 1      | yes           | lying | 1         | occupied, seen |
      Then in sample 11 the bed posture is "lying", seen "headboard_lying"

    Scenario: Back at the headboard, lying again
      Given the resident came in alone and lay down in samples 1 to 10
      When the camera sees this sequence:
        | samples | on floor | where | in bed | subject moves | pose        | occupants | bed            |
        | 11      | 0        |       | 1      | yes           | lying       | 1         | occupied, seen |
        | 12      | 0        |       | 1      | yes           | leaning out | 1         | occupied, seen |
        | 13      | 0        |       | 1      | yes           | lying       | 1         | occupied, seen |
      Then in sample 12 the bed posture is "getting_out"
      And in sample 13 the bed posture is "lying", seen "headboard_lying"

    Scenario: Sitting up against the headboard
      Given the resident came in alone and lay down in samples 1 to 10
      When the camera sees this sequence:
        | samples | on floor | where | in bed | subject moves | pose                            | occupants | bed            |
        | 11      | 0        |       | 1      | yes           | lying                           | 1         | occupied, seen |
        | 12      | 0        |       | 1      | yes           | sitting up against the headboard | 1         | occupied, seen |
      Then in sample 12 the bed posture is "sitting_up", seen "headboard_upright"
