# language: en
Feature: Posture by disparity
  As care staff
  I want to know whether the resident is lying, sitting or standing from how their head, torso and hips are ordered in depth
  to see the transition in which a fall happens

  Background:
    Given a room with the bed delimited
    And stabilization requires 2 seconds of source time
    And samples arrive every 1 second of source time
    And there is a pose model
    And there is a depth model
    And the motion threshold is 0.1 of the subject's diagonal
    And the resident came in alone and lay down in samples 1 to 10

  Scenario: Lying: the hips closer to the camera than the head
    When the camera sees this sequence:
      | samples | on floor | where | in bed | subject moves | pose  | occupants | bed            |
      | 11      | 0        |       | 1      | yes           | lying | 1         | occupied, seen |
    Then in sample 11 the posture is lying
    And in sample 11 the posture disparity is positive

  Scenario: Sitting up: head, torso and hips level out and they are upright
    When the camera sees this sequence:
      | samples | on floor | where | in bed | subject moves | pose       | occupants | bed            |
      | 11      | 0        |       | 1      | yes           | sitting up | 1         | occupied, seen |
    Then in sample 11 the posture is upright

  Scenario: Standing: even with a large disparity, if it is not lying it is upright
    When the camera sees this sequence:
      | samples | on floor | where | in bed | subject moves | pose     | occupants | bed            |
      | 11      | 0        |       | 1      | yes           | standing | 1         | occupied, seen |
    Then in sample 11 the posture is upright
    And in sample 11 the posture disparity is negative

  Scenario: With the feet covered, the hips are enough
    When the camera sees this sequence:
      | samples | on floor | where | in bed | subject moves | pose                | occupants | bed            |
      | 11      | 0        |       | 1      | yes           | lying, feet covered | 1         | occupied, seen |
    Then in sample 11 the posture is lying

  Scenario: Still: the posture holds with the pose
    When the camera sees this sequence:
      | samples | on floor | where | in bed | subject moves | pose  | occupants | bed            |
      | 11      | 0        |       | 1      | yes           | lying | 1         | occupied, seen |
      | 12-20   | 0        |       | 1      |               |       | 1         | occupied, seen |
    Then in sample 20 the posture is lying
    And in sample 20 the posture is stable since sample 11

  Scenario: The posture changes no count
    When the camera sees this sequence:
      | samples | on floor | where | in bed | subject moves | pose     | occupants | bed            |
      | 11      | 0        |       | 1      | yes           | standing | 1         | occupied, seen |
      | 12-15   | 0        |       | 1      |               |          | 1         | occupied, seen |
    Then in sample 15 the posture is upright
    And no exit from the bed was observed

  Scenario: A night replays without a camera and gives the same
    When the camera sees this sequence:
      | samples | on floor | where | in bed | subject moves | pose       | occupants | bed            |
      | 11      | 0        |       | 1      | yes           | lying      | 1         | occupied, seen |
      | 12-14   | 0        |       | 1      |               |            | 1         | occupied, seen |
      | 15      | 0        |       | 1      | yes           | sitting up | 1         | occupied, seen |
      | 16      | 0        |       | 1      | yes           | standing   | 1         | occupied, seen |
      | 17-20   | 0        |       | 1      |               |            | 1         | occupied, seen |
    Then the same night, replayed with the same readings, gives the same scenes and events
