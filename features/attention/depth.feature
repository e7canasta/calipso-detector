# language: en
Feature: The subject depth policy
  As care staff
  I want the engine, every time it takes the resident's posture, to also measure how far their body is
  to know later whether they are on the bed or in front of it, without paying for depth when there is no posture to measure

  Background:
    Given a room with the bed delimited
    And stabilization requires 2 seconds of source time
    And samples arrive every 1 second of source time
    And there is a pose model
    And there is a depth model
    And the motion threshold is 0.1 of the subject's diagonal

  Scenario: With a new pose, depth is measured in the same turn
    Given the resident came in alone and lay down in samples 1 to 10
    When the camera sees this sequence:
      | samples | on floor | where | in bed | subject moves | occupants | bed            |
      | 11      | 0        |       | 1      | yes           | 1         | occupied, seen |
    Then in sample 11 the looks were "detect, pose, depth"
    And in sample 11 the subject's depth is new

  Scenario: Without a depth crop, depth is measured over the whole frame
    Given the resident came in alone and lay down in samples 1 to 10
    When the camera sees this sequence:
      | samples | on floor | where | in bed | subject moves | occupants | bed            |
      | 11      | 0        |       | 1      | yes           | 1         | occupied, seen |
    Then in sample 11 depth was requested over the whole frame

  Scenario: With a depth crop, the subject's depth is measured over the bed, centred on it
    Given depth is measured over the bed grown by 0.25
    And the resident came in alone and lay down in samples 1 to 10
    When the camera sees this sequence:
      | samples | on floor | where | in bed | subject moves | occupants | bed            |
      | 11      | 0        |       | 1      | yes           | 1         | occupied, seen |
    Then in sample 11 depth was requested over the bed grown by 0.25

  Scenario: Only at the keypoints with enough score
    Given the resident came in alone and lay down in samples 1 to 10
    And the pose has 9 keypoints with enough score out of 17
    When the camera sees this sequence:
      | samples | on floor | where | in bed | subject moves | occupants | bed            |
      | 11      | 0        |       | 1      | yes           | 1         | occupied, seen |
    Then in sample 11 depth was requested at 9 keypoints and 1 bed point

  Scenario: Still: depth stays stable with the pose
    Given the resident came in alone and lay down in samples 1 to 10
    When the camera sees this sequence:
      | samples | on floor | where | in bed | occupants | bed            |
      | 11-20   | 0        |       | 1      | 1         | occupied, seen |
    Then in sample 20 the looks were "detect"
    And in sample 20 the subject's depth is stable since before sample 11

  Scenario: Without good keypoints it is not measured, and it says why
    Given the resident came in alone and lay down in samples 1 to 10
    And the pose has 0 keypoints with enough score out of 17
    When the camera sees this sequence:
      | samples | on floor | where | in bed | subject moves | occupants | bed            |
      | 11      | 0        |       | 1      | yes           | 1         | occupied, seen |
    Then in sample 11 depth was not requested because there is no pose
    And in sample 11 the subject has no depth

  Scenario: Depth fails: the pose is published anyway
    Given the resident came in alone and lay down in samples 1 to 10
    When the camera sees this sequence:
      | samples | on floor | where | in bed | subject moves | depth | occupants | bed            |
      | 11      | 0        |       | 1      | yes           | fails | 1         | occupied, seen |
    Then in sample 11 the pose is new
    And in sample 11 the subject has no depth

  Scenario: Without a depth model, the engine neither requests nor records it
    Given there is no depth model
    And the resident came in alone and lay down in samples 1 to 10
    When the camera sees this sequence:
      | samples | on floor | where | in bed | subject moves | occupants | bed            |
      | 11      | 0        |       | 1      | yes           | 1         | occupied, seen |
    Then in sample 11 the looks were "detect, pose"
    And in sample 11 the subject has no depth

  Scenario: A night with depth replays without a camera and gives the same
    Given the resident came in alone and lay down in samples 1 to 10
    When the camera sees this sequence:
      | samples | on floor | where | in bed | subject moves | depth | occupants | bed            |
      | 11-14   | 0        |       | 1      |               |       | 1         | occupied, seen |
      | 15      | 0        |       | 1      | yes           | fails | 1         | occupied, seen |
      | 16-20   | 0        |       | 1      | yes           |       | 1         | occupied, seen |
    Then the same night, replayed with the same readings, gives the same scenes and events
