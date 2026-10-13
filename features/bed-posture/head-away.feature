# language: en
Feature: The bed posture: the head away from the headboard
  As care staff
  I want to know when the resident left lying sits up, and nothing while they get into bed
  so as to go when they sit up, sliding down included

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

  Rule: Once lying, the head seen away from the headboard is not lying

    Scenario: Slid down the bed: the head below the headboard is sitting up, whatever the disparity
      Given the resident came in alone and lay down in samples 1 to 10
      When the camera sees this sequence:
        | samples | on floor | where | in bed | subject moves | pose      | occupants | bed            |
        | 11      | 0        |       | 1      | yes           | lying     | 1         | occupied, seen |
        | 12      | 0        |       | 1      | yes           | slid down | 1         | occupied, seen |
      Then in sample 12 the posture is lying
      And in sample 12 the bed posture is "sitting_up", seen "head_away"

    Scenario: Entering the bed nothing is reported, wherever the head is
      Given the resident came in alone and lay down in samples 1 to 10
      When the camera sees this sequence:
        | samples | on floor | where | in bed | subject moves | pose       | occupants | bed            |
        | 11      | 0        |       | 1      | yes           | sitting up | 1         | occupied, seen |
      Then in sample 11 the bed posture is "in_bed", seen "head_away"

    Scenario: Lying, they sit up: the head leaves the headboard
      Given the resident came in alone and lay down in samples 1 to 10
      When the camera sees this sequence:
        | samples | on floor | where | in bed | subject moves | pose       | occupants | bed            |
        | 11-20   | 0        |       | 0      |               |            | 1         | occupied, held |
        | 21-23   | 0        |       | 1      |               | lying      | 1         | occupied, seen |
        | 24      | 0        |       | 1      | yes           | sitting up | 1         | occupied, seen |
        | 25-26   | 0        |       | 1      |               |            | 1         | occupied, seen |
      Then in sample 23 the bed posture is "lying", seen "unknown"
      And in sample 24 the bed posture is "sitting_up", seen "head_away"
      And in sample 26 the bed posture is "sitting_up"

    Scenario: Without a depth model, the geometry still says sitting up
      Given there is no depth model
      And the resident came in alone and lay down in samples 1 to 10
      When the camera sees this sequence:
        | samples | on floor | where | in bed | subject moves | pose       | occupants | bed            |
        | 11-20   | 0        |       | 0      |               |            | 1         | occupied, held |
        | 21-23   | 0        |       | 1      |               | lying      | 1         | occupied, seen |
        | 24      | 0        |       | 1      | yes           | sitting up | 1         | occupied, seen |
      Then in sample 24 the bed posture is "sitting_up", seen "head_away"
