# language: en
Feature: Each change of the bed posture is published as an event
  As someone reading a night, or a consumer that only listens to events
  I want each layer-1 change as a row with its before, what was seen and its after
  to know the resident they left lying sat up or is getting out without reading every scene

  # Layer 1 publishes like layer 0 and the parts: a change of the stable
  # state is an event, its cell is a row of contract/bed-posture-table.txt
  # (before | seen | after), and nothing else is. Starting and stopping to
  # follow are not rows: getting into and out of bed are layer 0's events.

  Background:
    Given a room with the bed delimited
    And the bed's headboard delimited
    And stabilization requires 2 seconds of source time
    And samples arrive every 1 second of source time
    And there is a pose model
    And there is a depth model
    And the motion threshold is 0.1 of the subject's diagonal
    And the bed posture is followed

  Scenario: Held, it settles lying; then the head goes past the side: getting out
    Given the resident came in alone and lay down in samples 1 to 10
    When the camera sees this sequence:
      | samples | on floor | where | in bed | subject moves | pose        | occupants | bed            |
      | 11-20   | 0        |       | 0      |               |             | 1         | occupied, held |
      | 21-23   | 0        |       | 1      |               | lying       | 1         | occupied, seen |
      | 24      | 0        |       | 1      | yes           | leaning out | 1         | occupied, seen |
    Then in sample 13 the bed posture changed from "in_bed" to "lying", seen "held"
    And the bed posture changes published in samples 14 to 23 are none
    And in sample 24 the bed posture changed from "lying" to "getting_out", seen "head_side"

  Scenario: Leaving the bed and coming back publishes no bed posture change
    Given the resident came in alone and lay down in samples 1 to 10
    When the camera sees this sequence:
      | samples | on floor | where    | in bed | pose       | occupants | bed            |
      | 11-20   | 0        |          | 0      |            | 1         | occupied, held |
      | 21-23   | 0        |          | 1      | lying      | 1         | occupied, seen |
      | 24-28   | 1        | near bed | 0      |            | 1         | empty          |
      | 29-38   | 0        |          | 1      | sitting up | 1         | occupied, seen |
    Then in sample 28 there is no bed posture
    And in sample 38 the bed posture is "in_bed"
    And in sample 13 the bed posture changed from "in_bed" to "lying", seen "held"
    And the bed posture changes published in samples 14 to 38 are none

  Scenario: Not followed, nothing is published
    Given the bed posture is not followed
    And the resident came in alone and lay down in samples 1 to 10
    When the camera sees this sequence:
      | samples | on floor | where | in bed | subject moves | pose        | occupants | bed            |
      | 11-20   | 0        |       | 0      |               |             | 1         | occupied, held |
      | 21-23   | 0        |       | 1      |               | lying       | 1         | occupied, seen |
      | 24      | 0        |       | 1      | yes           | leaning out | 1         | occupied, seen |
    Then the bed posture changes published in samples 1 to 24 are none

  Scenario: Every bed posture change agrees with its scene
    Given the resident came in alone and lay down in samples 1 to 10
    When the camera sees this sequence:
      | samples | on floor | where | in bed | subject moves | pose        | occupants | bed            |
      | 11-20   | 0        |       | 0      |               |             | 1         | occupied, held |
      | 21-23   | 0        |       | 1      |               | lying       | 1         | occupied, seen |
      | 24      | 0        |       | 1      | yes           | leaning out | 1         | occupied, seen |
      | 25-30   | 0        |       | 1      |               | lying       | 1         | occupied, seen |
    Then on every turn, each bed posture change agrees with its scene
