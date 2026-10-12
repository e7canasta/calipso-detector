# language: en
Feature: The body groups in the scene
  As a consumer of calipso.scene.v4
  I want the posture to carry its groups (head, shoulders, pelvis), their disparities and the tolerance
  to see what the decision was made with, without today's scenes changing

  # R16, 1604 and 1606. Groups take the median depth of their seen keypoints
  # (score >= the profile's minimum) and say how many they saw of how many.

  Background:
    Given a room with the bed delimited
    And stabilization requires 2 seconds of source time
    And samples arrive every 1 second of source time
    And there is a pose model
    And there is a depth model
    And the motion threshold is 0.1 of the subject's diagonal

  Scenario: Each group says how many of its keypoints it saw
    Given the resident came in alone and lay down in samples 1 to 10
    When the camera sees this sequence:
      | samples | on floor | where | in bed | subject moves | pose  | occupants | bed            |
      | 11      | 0        |       | 1      | yes           | lying | 1         | occupied, seen |
    Then in sample 11 the group "head" saw 5 of 5 keypoints
    And in sample 11 the group "shoulders" saw 2 of 2 keypoints
    And in sample 11 the group "pelvis" saw 2 of 2 keypoints

  Scenario: A keypoint that is not seen does not count in its group
    Given the resident came in alone and lay down in samples 1 to 10
    When the camera sees this sequence:
      | samples | on floor | where | in bed | subject moves | pose     | occupants | bed            |
      | 11      | 0        |       | 1      | yes           | no torso | 1         | occupied, seen |
    Then in sample 11 the group "shoulders" saw 0 of 2 keypoints
    And in sample 11 the group "shoulders" has no depth

  Scenario: The scene publishes the three disparities and the tolerance
    Given the resident came in alone and lay down in samples 1 to 10
    When the camera sees this sequence:
      | samples | on floor | where | in bed | subject moves | pose       | occupants | bed            |
      | 11      | 0        |       | 1      | yes           | sitting up | 1         | occupied, seen |
    Then in sample 11 the disparities "upper_pelvis, shoulders_pelvis, head_pelvis" are negative
    And in sample 11 the posture tolerance is 0.05
    And in sample 11 the disparity "upper_pelvis" is the posture disparity

  Scenario: Today's posture fields keep coming out
    Given the resident came in alone and lay down in samples 1 to 10
    When the camera sees this sequence:
      | samples | on floor | where | in bed | subject moves | pose  | occupants | bed            |
      | 11      | 0        |       | 1      | yes           | lying | 1         | occupied, seen |
    Then in sample 11 the posture still publishes upper, lower and disparity

  Scenario: Without a depth model there is no posture and so no groups
    Given there is no depth model
    And the resident came in alone and lay down in samples 1 to 10
    When the camera sees this sequence:
      | samples | on floor | where | in bed | subject moves | pose  | occupants | bed            |
      | 11      | 0        |       | 1      | yes           | lying | 1         | occupied, seen |
    Then in sample 11 the subject publishes no "posture" key
