# language: en
Feature: The depth of each keypoint
  As whoever analyses the resident's posture
  I want each pose keypoint published with its name and its depth
  to compare head, torso and hips with each other, instead of receiving a single median

  Background:
    Given a room with the bed delimited
    And stabilization requires 2 seconds of source time
    And samples arrive every 1 second of source time
    And there is a pose model
    And there is a depth model
    And the motion threshold is 0.1 of the subject's diagonal
    And the pose has 17 keypoints with enough score out of 17

  Scenario: Each keypoint is published with its COCO name
    Given the resident came in alone and lay down in samples 1 to 10
    When the camera sees this sequence:
      | samples | on floor | where | in bed | subject moves | occupants | bed            |
      | 11      | 0        |       | 1      | yes           | 1         | occupied, seen |
    Then in sample 11 the pose keypoints are named, in order, as the 17 of COCO

  Scenario: Only measured keypoints carry depth, each its own
    Given the resident came in alone and lay down in samples 1 to 10
    And the pose has 9 keypoints with enough score out of 17
    When the camera sees this sequence:
      | samples | on floor | where | in bed | subject moves | occupants | bed            |
      | 11      | 0        |       | 1      | yes           | 1         | occupied, seen |
    Then in sample 11 exactly the keypoints with enough score carry depth
    And in sample 11 each measured keypoint carries the depth read at it

  Scenario: Still: stable keypoints keep their depth
    Given the resident came in alone and lay down in samples 1 to 10
    When the camera sees this sequence:
      | samples | on floor | where | in bed | subject moves | occupants | bed            |
      | 11      | 0        |       | 1      | yes           | 1         | occupied, seen |
      | 12-20   | 0        |       | 1      |               | 1         | occupied, seen |
    Then in sample 20 the pose keypoints are those of sample 11, with their depth

  Scenario: Without a depth model, keypoints go without depth
    Given there is no depth model
    And the resident came in alone and lay down in samples 1 to 10
    When the camera sees this sequence:
      | samples | on floor | where | in bed | subject moves | occupants | bed            |
      | 11      | 0        |       | 1      | yes           | 1         | occupied, seen |
    Then in sample 11 the pose keypoints are named, in order, as the 17 of COCO
    And in sample 11 no keypoint carries depth

  Scenario: Depth fails: keypoints go without depth and the pose is published
    Given the resident came in alone and lay down in samples 1 to 10
    When the camera sees this sequence:
      | samples | on floor | where | in bed | subject moves | depth | occupants | bed            |
      | 11      | 0        |       | 1      | yes           | fails | 1         | occupied, seen |
    Then in sample 11 the pose is new
    And in sample 11 no keypoint carries depth

  Scenario: The subject's median is still published
    Given the resident came in alone and lay down in samples 1 to 10
    When the camera sees this sequence:
      | samples | on floor | where | in bed | subject moves | occupants | bed            |
      | 11      | 0        |       | 1      | yes           | 1         | occupied, seen |
    Then in sample 11 the subject's depth is the median of their keypoints

  Scenario: A night with measured keypoints replays without a camera and gives the same
    Given the resident came in alone and lay down in samples 1 to 10
    And the pose has 9 keypoints with enough score out of 17
    When the camera sees this sequence:
      | samples | on floor | where | in bed | subject moves | depth | occupants | bed            |
      | 11-14   | 0        |       | 1      |               |       | 1         | occupied, seen |
      | 15      | 0        |       | 1      | yes           | fails | 1         | occupied, seen |
      | 16-18   | 0        |       | 0      |               |       | 1         | occupied, held |
      | 19-22   | 0        |       | 1      | yes           |       | 1         | occupied, seen |
    Then the same night, replayed with the same readings, gives the same scenes and events
