# language: en
Feature: The bed posture holds its time
  As care staff
  I want a new bed posture to count once it held, each state its own time
  so as not to be called for a flicker, and to be called sooner for getting out

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

  Rule: A new bed posture counts once it held, each state its own source time

    Scenario: Sitting up must hold before it counts; still, it keeps holding
      Given the bed posture "sitting_up" must hold 3 seconds of source time
      And the resident came in alone and lay down in samples 1 to 10
      When the camera sees this sequence:
        | samples | on floor | where | in bed | subject moves | pose       | occupants | bed            |
        | 11      | 0        |       | 1      | yes           | lying      | 1         | occupied, seen |
        | 12      | 0        |       | 1      | yes           | sitting up | 1         | occupied, seen |
        | 13-14   | 0        |       | 1      |               |            | 1         | occupied, seen |
        | 15      | 0        |       | 1      |               |            | 1         | occupied, seen |
      Then in sample 14 the bed posture is "lying"
      And in sample 15 the bed posture is "sitting_up"

    Scenario: A flicker back to lying drops the candidate
      Given the bed posture "sitting_up" must hold 3 seconds of source time
      And the resident came in alone and lay down in samples 1 to 10
      When the camera sees this sequence:
        | samples | on floor | where | in bed | subject moves | pose       | occupants | bed            |
        | 11      | 0        |       | 1      | yes           | lying      | 1         | occupied, seen |
        | 12      | 0        |       | 1      | yes           | sitting up | 1         | occupied, seen |
        | 13      | 0        |       | 1      | yes           | lying      | 1         | occupied, seen |
        | 14-16   | 0        |       | 1      |               |            | 1         | occupied, seen |
      Then in sample 16 the bed posture is "lying"

    Scenario: Getting out counts sooner than sitting up
      Given the bed posture "sitting_up" must hold 3 seconds of source time
      And the bed posture "getting_out" must hold 1 seconds of source time
      And the resident came in alone and lay down in samples 1 to 10
      When the camera sees this sequence:
        | samples | on floor | where | in bed | subject moves | pose        | occupants | bed            |
        | 11      | 0        |       | 1      | yes           | lying       | 1         | occupied, seen |
        | 12      | 0        |       | 1      | yes           | leaning out | 1         | occupied, seen |
        | 13      | 0        |       | 1      |               |             | 1         | occupied, seen |
      Then in sample 12 the bed posture is "lying"
      And in sample 13 the bed posture is "getting_out"
