# language: en
Feature: The bed's points, measured with the body
  As the second level
  I want the depth of fixed points of the bed, read in the same look as the body
  to stand in for the hips when the blanket hides them

  # R17. The bed's points are measured with the resident in bed, in the same
  # depth map as their keypoints: covered, the bed's centre is the blanket over
  # the hips. Never the empty bed's calibration: the model's scale is relative
  # and changes from frame to frame.

  Background:
    Given a room with the bed delimited
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
    Then in sample 11 the bed point "centre" was measured at the bed's centre
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

  Scenario: Sitting up, the head comes closer than the bed's centre
    Given the resident came in alone and lay down in samples 1 to 10
    When the camera sees this sequence:
      | samples | on floor | where | in bed | subject moves | pose       | occupants | bed            |
      | 11      | 0        |       | 1      | yes           | sitting up | 1         | occupied, seen |
    Then in sample 11 the disparities "upper_pelvis, upper_bed_centre" are negative
