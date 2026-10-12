# language: en
Feature: The bed's points, measured with the body
  As the second level
  I want the depth of fixed points of the bed, read in the same look as the body
  to stand in for the hips when the blanket hides them

  # R17, 1707. The bed's points are measured with the resident in bed, in the
  # same depth map as their keypoints: covered, the bed's centre is the blanket
  # over the hips. Never the empty bed's calibration: the model's scale is
  # relative and changes from frame to frame.
  #
  # The stand-in is situational: the bed's centre is the hips only with the
  # head at the headboard. Where the head is, is a fact of its own (head_place),
  # concluded today by the face or the pose's head inside the headboard region;
  # the bed's centre depends only on that fact, not on how it was concluded.

  Background:
    Given a room with the bed delimited
    And the bed's headboard delimited
    And stabilization requires 2 seconds of source time
    And samples arrive every 1 second of source time
    And there is a pose model
    And there is a depth model
    And the motion threshold is 0.1 of the subject's diagonal

  Scenario: Lying, the bed's centre is measured in the same look as the body
    Given the resident came in alone and lay down in samples 1 to 10
    When the camera sees this sequence:
      | samples | on floor | where | in bed | subject moves | pose  | occupants | bed            |
      | 11      | 0        |       | 1      | yes           | lying | 1         | occupied, seen |
    Then in sample 11 the head is "at_headboard", by "pose_head"
    And in sample 11 the bed point "centre" was measured at the bed's centre
    And in sample 11 the head is farther from the camera than the bed point "centre"

  # Both are computed and published: against the pelvis (it decides today) and
  # against the bed's centre (the stand-in), to compare them on every night.
  Scenario: Lying, the disparity against the bed's centre is published with the pelvis'
    Given the resident came in alone and lay down in samples 1 to 10
    When the camera sees this sequence:
      | samples | on floor | where | in bed | subject moves | pose  | occupants | bed            |
      | 11      | 0        |       | 1      | yes           | lying | 1         | occupied, seen |
    Then in sample 11 the disparities "upper_pelvis, upper_bed_centre" are positive
    And in sample 11 the disparity "upper_pelvis" is the posture disparity

  Scenario: The head beyond the headboard, the bed's centre is not the hips: not measured
    Given the resident came in alone and lay down in samples 1 to 10
    When the camera sees this sequence:
      | samples | on floor | where | in bed | subject moves | pose       | occupants | bed            |
      | 11      | 0        |       | 1      | yes           | sitting up | 1         | occupied, seen |
    Then in sample 11 the head is "elsewhere", by "pose_head"
    And in sample 11 the posture publishes no bed points
    And in sample 11 the disparities "upper_pelvis" are negative

  Scenario: Without the headboard delimited, where the head is is unknown: no stand-in
    Given the bed's headboard is not delimited
    And the resident came in alone and lay down in samples 1 to 10
    When the camera sees this sequence:
      | samples | on floor | where | in bed | subject moves | pose  | occupants | bed            |
      | 11      | 0        |       | 1      | yes           | lying | 1         | occupied, seen |
    Then in sample 11 the head is "unknown"
    And in sample 11 the posture publishes no bed points
