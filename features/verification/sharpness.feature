# language: en
Feature: Only sharp crops count for a burst
  As care staff
  I want the doubt asked with sharp frames, not blurred ones, so that the
  verifier and the room's own head decide on what can be seen
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
    And a crop counts if it is 0.5 of the median of the camera's last 20, waiting at most 6 seconds
    And the verifier answers after 3 seconds of source time

  Scenario: A blurred crop does not count: the burst waits for a sharp one
    Given the resident came in alone and lay down in samples 1 to 10
    And the verifier answers "yes"
    And the crops of samples 14 to 14 are blurred
    When the camera sees this sequence:
      | samples | on floor | where | in bed | subject moves | pose                             | occupants | bed            |
      | 11      | 0        |       | 1      | yes           | lying                            | 1         | occupied, seen |
      | 12      | 0        |       | 1      | yes           | sitting up against the headboard | 1         | occupied, seen |
      | 13-22   | 0        |       | 1      |               |                                  | 1         | occupied, seen |
    Then in sample 12 a crop is kept for the verifier
    And in sample 14 a blurred crop is kept, not counted
    And in sample 16 a crop is kept for the verifier
    And in sample 16 nothing is asked to the verifier
    And in sample 18 a verification of the disparity is asked
    And in sample 21 the bed posture is "sitting_up", seen "headboard_upright"

  # The threshold is relative: a camera that stays blurred lowers its own
  # median, and its crops start to count. Here the longest wait comes first.
  Scenario: Never sharp enough: after the longest wait it asks with the sharpest
    Given the resident came in alone and lay down in samples 1 to 10
    And the verifier answers "yes"
    And the crops of samples 14 to 30 are blurred
    When the camera sees this sequence:
      | samples | on floor | where | in bed | subject moves | pose                             | occupants | bed            |
      | 11      | 0        |       | 1      | yes           | lying                            | 1         | occupied, seen |
      | 12      | 0        |       | 1      | yes           | sitting up against the headboard | 1         | occupied, seen |
      | 13-22   | 0        |       | 1      |               |                                  | 1         | occupied, seen |
    Then in sample 16 nothing is asked to the verifier
    And in sample 18 a verification of the disparity is asked
