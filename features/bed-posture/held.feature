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
