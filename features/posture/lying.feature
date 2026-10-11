# language: en
Feature: Truly lying
  As care staff
  I want to know whether the resident is stretched out on the mattress, and not only that their body is in the bed zone
  to tell someone lying from someone sitting on the edge or sitting up

  Background:
    Given a room with the bed delimited
    And stabilization requires 2 seconds of source time
    And samples arrive every 1 second of source time
    And there is a pose model
    And the motion threshold is 0.1 of the subject's diagonal
    And the resident came in alone and lay down in samples 1 to 10

  Scenario: Lying with the feet in sight: confirmed
    When the camera sees this sequence:
      | samples | on floor | where | in bed | subject moves | pose  | occupants | bed            |
      | 11      | 0        |       | 1      | yes           | lying | 1         | occupied, seen |
    Then in sample 11 they are truly lying, confirmed

  Scenario: Lying with the feet covered: assumed
    When the camera sees this sequence:
      | samples | on floor | where | in bed | subject moves | pose                | occupants | bed            |
      | 11      | 0        |       | 1      | yes           | lying, feet covered | 1         | occupied, seen |
    Then in sample 11 they are truly lying, assumed

  Scenario: An arm off the bed changes nothing
    When the camera sees this sequence:
      | samples | on floor | where | in bed | subject moves | pose       | occupants | bed            |
      | 11      | 0        |       | 1      | yes           | an arm out | 1         | occupied, seen |
    Then in sample 11 they are truly lying, confirmed

  Scenario: Sitting on the edge with the legs off: not lying
    When the camera sees this sequence:
      | samples | on floor | where | in bed | subject moves | pose                | occupants | bed            |
      | 11      | 0        |       | 1      | yes           | sitting on the edge | 1         | occupied, seen |
    Then in sample 11 they are not lying

  Scenario: Sitting up, with the torso off the plane: not lying
    When the camera sees this sequence:
      | samples | on floor | where | in bed | subject moves | pose       | occupants | bed            |
      | 11      | 0        |       | 1      | yes           | sitting up | 1         | occupied, seen |
    Then in sample 11 they are not lying

  Scenario: Without the torso in sight it is unknown
    When the camera sees this sequence:
      | samples | on floor | where | in bed | subject moves | pose     | occupants | bed            |
      | 11      | 0        |       | 1      | yes           | no torso | 1         | occupied, seen |
    Then in sample 11 it is unknown whether they are lying

  Scenario: Still: it holds with the pose, stable
    When the camera sees this sequence:
      | samples | on floor | where | in bed | subject moves | pose  | occupants | bed            |
      | 11      | 0        |       | 1      | yes           | lying | 1         | occupied, seen |
      | 12-20   | 0        |       | 1      |               |       | 1         | occupied, seen |
    Then in sample 20 they are truly lying, confirmed
    And in sample 20 lying is stable since sample 11

  Scenario: Truly lying changes no count
    When the camera sees this sequence:
      | samples | on floor | where | in bed | subject moves | pose       | occupants | bed            |
      | 11      | 0        |       | 1      | yes           | sitting up | 1         | occupied, seen |
      | 12-15   | 0        |       | 1      |               |            | 1         | occupied, seen |
    Then in sample 15 they are not lying
    And no exit from the bed was observed

  Scenario: A night replays without a camera and gives the same
    When the camera sees this sequence:
      | samples | on floor | where | in bed | subject moves | pose                | occupants | bed            |
      | 11-12   | 0        |       | 1      | yes           | lying, feet covered | 1         | occupied, seen |
      | 13-15   | 0        |       | 1      |               |                     | 1         | occupied, seen |
      | 16      | 0        |       | 1      | yes           | sitting on the edge | 1         | occupied, seen |
      | 17-20   | 0        |       | 1      |               |                     | 1         | occupied, seen |
    Then the same night, replayed with the same readings, gives the same scenes and events
