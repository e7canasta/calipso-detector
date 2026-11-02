# language: en

Feature: A disparity burst at its edges
  As care staff
  I want a doubtful disparity asked with a burst to behave the same at its
  edges: a stale burst that gives its question back to the budget, a question
  that went out and keeps it, a request that is still in flight, and the time
  a request has to run out, so that the budget and the bed posture are never
  spent or changed by a question that did not happen (white book, volume III,
  ch. 12)

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

  Scenario: A burst cut by a new stream is dropped, not asked
    Given the resident came in alone and lay down in samples 1 to 10
    And the verifier answers "yes"
    And the camera reconnects before sample 13
    When the camera sees this sequence:
      | samples | on floor | where | in bed | subject moves | pose                             | occupants | bed            |
      | 11      | 0        |       | 1      | yes           | lying                            | 1         | occupied, seen |
      | 12      | 0        |       | 1      | yes           | sitting up against the headboard | 1         | occupied, seen |
      | 13-16   | 0        |       | 1      |               |                                  | 1         | occupied, seen |
    Then in sample 12 a crop is kept for the verifier
    And in sample 13 the burst is dropped, stale
    And in sample 16 nothing is asked to the verifier

  Scenario: A doubt that comes while a question is in flight waits: the verifier is busy
    Given the disparity is asked with 1 frames 0 seconds apart
    And the resident came in alone and lay down in samples 1 to 10
    And the verifier answers "yes"
    When the camera sees this sequence:
      | samples | on floor | where | in bed | subject moves | pose                             | occupants | bed            |
      | 11      | 0        |       | 1      | yes           | lying                            | 1         | occupied, seen |
      | 12      | 0        |       | 1      | yes           | sitting up against the headboard | 1         | occupied, seen |
      | 13-14   | 0        |       | 1      | yes           | sitting up against the headboard | 1         | occupied, seen |
    Then in sample 12 a verification of the disparity is asked
    And in sample 13 the verifier is busy

  Scenario: The time a burst waits for its answer is counted from the question, not from the first crop
    Given verification requests expire after 4 seconds
    And the verifier never answers
    And the resident came in alone and lay down in samples 1 to 10
    When the camera sees this sequence:
      | samples | on floor | where | in bed | subject moves | pose                             | occupants | bed            |
      | 11      | 0        |       | 1      | yes           | lying                            | 1         | occupied, seen |
      | 12      | 0        |       | 1      | yes           | sitting up against the headboard | 1         | occupied, seen |
      | 13-20   | 0        |       | 1      | yes           | sitting up against the headboard | 1         | occupied, seen |
    Then in sample 12 a crop is kept for the verifier
    And in sample 16 a verification of the disparity is asked
    And in sample 17 the verifier is busy
    And in sample 20 a crop is kept for the verifier
