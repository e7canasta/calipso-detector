# language: en
Feature: The bed posture: held in bed is lying
  As care staff
  I want the resident held in bed, unseen, to be lying
  so as not to lose the posture exactly when they cover themselves

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

  Rule: Held in bed is lying: upright is big and would be seen

    Scenario: Held in bed after lying down, the bed posture is lying
      Given the resident came in alone and lay down in samples 1 to 10
      When the camera sees this sequence:
        | samples | on floor | where | in bed | subject moves | pose       | occupants | bed            |
        | 11      | 0        |       | 1      | yes           | sitting up | 1         | occupied, seen |
        | 12-20   | 0        |       | 0      |               |            | 1         | occupied, held |
      Then in sample 11 the bed posture is "in_bed"
      And in sample 20 the bed posture is "lying", seen "held"
