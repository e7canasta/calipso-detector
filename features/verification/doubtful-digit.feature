# language: en
Feature: A doubtful digit is filled by the verifier
  As care staff
  I want a reading that falls in the camera's grey band to be asked to the
  slower verifier, instead of being believed on the wrong side
  so as to be called when they sit up against the headboard, where depth
  cannot tell (white book, volume III, ch. 10)

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
    And the verifier answers after 3 seconds of source time

  Scenario: Doubtful at the headboard, the verifier says sitting: sitting up
    Given the resident came in alone and lay down in samples 1 to 10
    And the verifier answers "yes"
    When the camera sees this sequence:
      | samples | on floor | where | in bed | subject moves | pose                             | occupants | bed            |
      | 11      | 0        |       | 1      | yes           | lying                            | 1         | occupied, seen |
      | 12      | 0        |       | 1      | yes           | sitting up against the headboard | 1         | occupied, seen |
      | 13-16   | 0        |       | 1      |               |                                  | 1         | occupied, seen |
    Then in sample 12 the bed posture is "lying", seen "unknown"
    And in sample 12 a verification of the disparity is asked
    And in sample 14 the bed posture is "lying"
    And in sample 15 the bed posture is "sitting_up", seen "headboard_upright"

  Scenario: Doubtful at the headboard, the verifier says lying: still lying
    Given the resident came in alone and lay down in samples 1 to 10
    And the verifier answers "no"
    When the camera sees this sequence:
      | samples | on floor | where | in bed | subject moves | pose                             | occupants | bed            |
      | 11      | 0        |       | 1      | yes           | lying                            | 1         | occupied, seen |
      | 12      | 0        |       | 1      | yes           | sitting up against the headboard | 1         | occupied, seen |
      | 13-16   | 0        |       | 1      |               |                                  | 1         | occupied, seen |
    Then in sample 16 the bed posture is "lying"

  Scenario: Out of the grey band nothing is asked
    Given the resident came in alone and lay down in samples 1 to 10
    When the camera sees this sequence:
      | samples | on floor | where | in bed | subject moves | pose  | occupants | bed            |
      | 11      | 0        |       | 1      | yes           | lying | 1         | occupied, seen |
    Then in sample 11 the bed posture is "lying", seen "headboard_lying"
    And in sample 11 nothing is asked to the verifier
