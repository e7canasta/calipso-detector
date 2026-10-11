# language: en
Feature: When to look again: the pose's maximum age
  As care staff
  I want the engine to take again, every so often, the posture of a resident who does not move
  so someone sedated is not described forever by a pose from hours ago

  Background:
    Given a room with the bed delimited
    And stabilization requires 2 seconds of source time
    And samples arrive every 1 second of source time
    And there is a pose model
    And the motion threshold is 0.1 of the subject's diagonal
    And the pose expires after 10 seconds of source time
    And the resident came in alone and lay down in samples 1 to 10

  Scenario: Sleeps still: the pose is renewed before it expires
    When the camera sees this sequence:
      | samples | on floor | where | in bed | occupants | bed            |
      | 11-40   | 0        |       | 1      | 1         | occupied, seen |
    Then between samples 11 and 40 the published pose is never 10 seconds old or more
    And between samples 11 and 40 the pose was requested 3 times

  Scenario: Held: age does not trigger a look
    When the camera sees this sequence:
      | samples | on floor | where | in bed | occupants | bed            |
      | 11-40   | 0        |       | 0      | 1         | occupied, held |
    Then the pose was not requested in samples 11 to 40

  Scenario: A night replays without a camera and gives the same
    When the camera sees this sequence:
      | samples | on floor | where | in bed | subject moves | occupants | bed            |
      | 11-25   | 0        |       | 1      |               | 1         | occupied, seen |
      | 26      | 0        |       | 1      | yes           | 1         | occupied, seen |
      | 27-30   | 0        |       | 0      |               | 1         | occupied, held |
    Then the same night, replayed with the same readings, gives the same scenes and events
