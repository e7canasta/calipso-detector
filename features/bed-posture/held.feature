# language: en
Feature: The bed posture: the resident they left lying
  As care staff
  I want to know whether the resident we left lying is still lying
  so as to go when they sit up or try to get out, and not when they lie down

  # Layer 1, the sister of the occupancy: a machine with its own golden table
  # (`contract/bed-posture-table.txt`), before : seen : after. It only reports
  # leaving lying: entering the bed, nothing (R18, 1806).

  Background:
    Given a room with the bed delimited
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
        | samples | on floor | where | in bed | subject moves | pose  | occupants | bed            |
        | 11      | 0        |       | 1      | yes           | lying | 1         | occupied, seen |
        | 12-20   | 0        |       | 0      |               |       | 1         | occupied, held |
      Then in sample 11 the bed posture is "in_bed"
      And in sample 20 the bed posture is "lying", seen "held"
