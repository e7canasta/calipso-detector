# language: en
Feature: The bed posture lives on layer 0
  As care staff
  I want the bed posture to exist only while the resident is alone in bed, and only when the profile follows it
  so that it reads layer 0 and never breaks it

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

  Rule: Layer 1 lives while layer 0 believes the resident alone in bed

    Scenario: Not followed, there is no bed posture
      Given the bed posture is not followed
      And the resident came in alone and lay down in samples 1 to 10
      When the camera sees this sequence:
        | samples | on floor | where | in bed | occupants | bed            |
        | 11-20   | 0        |       | 0      | 1         | occupied, held |
      Then in sample 20 there is no bed posture

    Scenario: Out of bed it forgets: coming back starts in bed again
      Given the resident came in alone and lay down in samples 1 to 10
      When the camera sees this sequence:
        | samples | on floor | where    | in bed | pose       | occupants | bed            |
        | 11-20   | 0        |          | 0      |            | 1         | occupied, held |
        | 21-23   | 0        |          | 1      | lying      | 1         | occupied, seen |
        | 24-28   | 1        | near bed | 0      |            | 1         | empty          |
        | 29-38   | 0        |          | 1      | sitting up | 1         | occupied, seen |
      Then in sample 20 the bed posture is "lying", seen "held"
      And in sample 23 the bed posture is "lying", seen "unknown"
      And in sample 28 there is no bed posture
      And in sample 38 the bed posture is "in_bed"
