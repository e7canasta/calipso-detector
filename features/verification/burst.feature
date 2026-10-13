# language: en
Feature: A doubt is asked with a burst of frames
  As care staff
  I want a doubtful reading asked with several frames over a few seconds,
  not one, so that the verifier sees the movement (sitting up or lying back,
  getting out or getting in) instead of a blurred instant
  (white book, volume III, ch. 12)

  Background:
    Given a room with the bed delimited
    And the bed's headboard delimited
    And stabilization requires 2 seconds of source time
    And samples arrive every 1 second of source time
    And there is a pose model
    And there is a depth model
    And the motion threshold is 0.1 of the subject's diagonal
    And the bed posture is followed
    And disparities between -0.2 and 0.0 are doubtful
    And the disparity is asked with 3 frames 2 seconds apart
    And the verifier answers after 3 seconds of source time

  Scenario: Three frames are kept, then asked once, and the answer fills the first
    Given the resident came in alone and lay down in samples 1 to 10
    And the verifier answers "yes"
    When the camera sees this sequence:
      | samples | on floor | where | in bed | subject moves | pose                             | occupants | bed            |
      | 11      | 0        |       | 1      | yes           | lying                            | 1         | occupied, seen |
      | 12      | 0        |       | 1      | yes           | sitting up against the headboard | 1         | occupied, seen |
      | 13-20   | 0        |       | 1      |               |                                  | 1         | occupied, seen |
    Then in sample 12 a crop is kept for the verifier
    And in sample 12 nothing is asked to the verifier
    And in sample 13 nothing is kept for the verifier
    And in sample 14 a crop is kept for the verifier
    And in sample 16 a verification of the disparity is asked
    And in sample 18 the bed posture is "lying"
    And in sample 19 the bed posture is "sitting_up", seen "headboard_upright"

  Scenario: The doubt clears while collecting: nothing is asked
    Given the resident came in alone and lay down in samples 1 to 10
    And the verifier answers "yes"
    When the camera sees this sequence:
      | samples | on floor | where | in bed | subject moves | pose                             | occupants | bed            |
      | 11      | 0        |       | 1      | yes           | lying                            | 1         | occupied, seen |
      | 12      | 0        |       | 1      | yes           | sitting up against the headboard | 1         | occupied, seen |
      | 13      | 0        |       | 1      | yes           | lying                            | 1         | occupied, seen |
      | 14-18   | 0        |       | 1      |               |                                  | 1         | occupied, seen |
    Then in sample 12 a crop is kept for the verifier
    And in sample 13 the burst is dropped, resolved
    And in sample 16 nothing is asked to the verifier
    And in sample 18 the bed posture is "lying"

  Scenario: Layer 1 moves on while collecting: the burst is dropped
    Given the resident came in alone and lay down in samples 1 to 10
    And the verifier answers "yes"
    When the camera sees this sequence:
      | samples | on floor | where | in bed | subject moves | pose                             | occupants | bed            |
      | 11      | 0        |       | 1      | yes           | lying                            | 1         | occupied, seen |
      | 12      | 0        |       | 1      | yes           | sitting up against the headboard | 1         | occupied, seen |
      | 13      | 0        |       | 1      | yes           | slid down                        | 1         | occupied, seen |
      | 14-18   | 0        |       | 1      |               |                                  | 1         | occupied, seen |
    Then in sample 12 a crop is kept for the verifier
    And in sample 13 the burst is dropped, stale
    And in sample 16 nothing is asked to the verifier

  Scenario: A burst counts once for the budget
    Given the resident came in alone and lay down in samples 1 to 10
    And the verifier answers "no"
    And the verifier may be asked 1 times per hour
    When the camera sees this sequence:
      | samples | on floor | where | in bed | subject moves | pose                             | occupants | bed            |
      | 11      | 0        |       | 1      | yes           | lying                            | 1         | occupied, seen |
      | 12      | 0        |       | 1      | yes           | sitting up against the headboard | 1         | occupied, seen |
      | 13-17   | 0        |       | 1      |               |                                  | 1         | occupied, seen |
    Then in sample 14 a crop is kept for the verifier
    And in sample 16 a verification of the disparity is asked
