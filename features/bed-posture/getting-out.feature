# language: en
Feature: The bed posture: getting out over the sides
  As care staff
  I want to know when the resident left lying puts the head or a foot past the bed's sides
  so as to go before they are out of bed

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

  Rule: Once lying, the head or the feet past the bed's sides are getting out

    Scenario: Lying, the head goes past the side of the bed
      Given the resident came in alone and lay down in samples 1 to 10
      When the camera sees this sequence:
        | samples | on floor | where | in bed | subject moves | pose        | occupants | bed            |
        | 11-20   | 0        |       | 0      |               |             | 1         | occupied, held |
        | 21-23   | 0        |       | 1      |               | lying       | 1         | occupied, seen |
        | 24      | 0        |       | 1      | yes           | leaning out | 1         | occupied, seen |
      Then in sample 24 the bed posture is "getting_out", seen "head_side"

    Scenario: Lying, the feet go past the side of the bed, with the head at the headboard
      Given the resident came in alone and lay down in samples 1 to 10
      When the camera sees this sequence:
        | samples | on floor | where | in bed | subject moves | pose                | occupants | bed            |
        | 11-20   | 0        |       | 0      |               |                     | 1         | occupied, held |
        | 21-23   | 0        |       | 1      |               | lying               | 1         | occupied, seen |
        | 24      | 0        |       | 1      | yes           | sitting on the edge | 1         | occupied, seen |
      Then in sample 24 the bed posture is "getting_out", seen "legs_out"

    Scenario: The feet win over the head away from the headboard
      Given the resident came in alone and lay down in samples 1 to 10
      When the camera sees this sequence:
        | samples | on floor | where | in bed | subject moves | pose                           | occupants | bed            |
        | 11-20   | 0        |       | 0      |               |                                | 1         | occupied, held |
        | 21-23   | 0        |       | 1      |               | lying                          | 1         | occupied, seen |
        | 24      | 0        |       | 1      | yes           | sitting on the edge, head down | 1         | occupied, seen |
      Then in sample 24 the bed posture is "getting_out", seen "legs_out"
