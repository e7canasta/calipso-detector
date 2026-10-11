# language: en
Feature: When to look again: a change of shape is motion too
  As care staff
  I want the engine to take the posture again when the body changes shape even if it does not shift
  so a pose taken standing does not describe someone who already lay down in the same place

  Background:
    Given a room with the bed delimited
    And stabilization requires 2 seconds of source time
    And samples arrive every 1 second of source time
    And there is a pose model
    And the motion threshold is 0.1 of the subject's diagonal
    And the resident came in alone and lay down in samples 1 to 10

  Scenario: Changes shape without shifting: the pose is requested
    When the camera sees this sequence:
      | samples | on floor | where | in bed | subject changes shape | occupants | bed            |
      | 11-14   | 0        |       | 1      |                       | 1         | occupied, seen |
      | 15      | 0        |       | 1      | yes                   | 1         | occupied, seen |
      | 16-20   | 0        |       | 1      |                       | 1         | occupied, seen |
    Then between samples 11 and 20 the pose was requested only in 15
    And in sample 20 the pose is stable since sample 15
