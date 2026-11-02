# language: en

Feature: The verifier's port can refuse a request or a crop
  As care staff
  I want a refused question or crop to close its burst at once, and nothing to
  stay owed to a verifier that refused it, so that the next doubt is asked
  again and no question waits for an answer that will never come (white book,
  volume III, ch. 12)

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

  Scenario: A refused request is not owed: the next doubt is asked again
    Given the disparity is asked with 1 frames 0 seconds apart
    And the verifier refuses to take its requests
    And the resident came in alone and lay down in samples 1 to 10
    And the verifier answers "yes"
    When the camera sees this sequence:
      | samples | on floor | where | in bed | subject moves | pose                             | occupants | bed            |
      | 11      | 0        |       | 1      | yes           | lying                            | 1         | occupied, seen |
      | 12      | 0        |       | 1      | yes           | sitting up against the headboard | 1         | occupied, seen |
      | 13-14   | 0        |       | 1      | yes           | sitting up against the headboard | 1         | occupied, seen |
    Then in sample 12 a verification of the disparity fails
    And in sample 13 a verification of the disparity fails

  Scenario: A refused crop closes its burst, and the next doubt starts a new one
    Given the disparity is asked with 3 frames 2 seconds apart
    And the verifier refuses to keep the crops
    And the resident came in alone and lay down in samples 1 to 10
    And the verifier answers "yes"
    When the camera sees this sequence:
      | samples | on floor | where | in bed | subject moves | pose                             | occupants | bed            |
      | 11      | 0        |       | 1      | yes           | lying                            | 1         | occupied, seen |
      | 12      | 0        |       | 1      | yes           | sitting up against the headboard | 1         | occupied, seen |
      | 13      | 0        |       | 1      | yes           | sitting up against the headboard | 1         | occupied, seen |
    Then in sample 12 a crop fails
    And in sample 13 a crop fails
