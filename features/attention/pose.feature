# language: en
Feature: The pose policy
  As care staff
  I want the engine to take the resident's posture when they move, and not on every sample while they sleep still
  to keep their posture up to date without spending the model while nothing changes

  Background:
    Given a room with the bed delimited
    And stabilization requires 2 seconds of source time
    And samples arrive every 1 second of source time
    And there is a pose model
    And the motion threshold is 0.1 of the subject's diagonal

  Scenario: Lies down: the first pose is requested without a reference
    When the camera sees this sequence:
      | samples | on floor | where | in bed | occupants | bed            |
      | 1-3     | 1        | door  | 0      | 1         | empty          |
      | 4-6     | 0        |       | 1      | 1         | occupied, seen |
    Then the first pose was requested without a reference, over the subject's box
    And the pose was requested again on moving from the door to the bed

  Scenario: Sleeps still: the pose is not requested again and is published as stable
    Given the resident came in alone and lay down in samples 1 to 10
    When the camera sees this sequence:
      | samples | on floor | where | in bed | occupants | bed            |
      | 11-30   | 0        |       | 1      | 1         | occupied, seen |
    Then the pose was not requested in samples 11 to 30
    And in sample 30 the pose is stable since before sample 11
    And in sample 30 the pose was not requested because the subject did not move

  Scenario: Turns over: the pose is requested again
    Given the resident came in alone and lay down in samples 1 to 10
    When the camera sees this sequence:
      | samples | on floor | where | in bed | subject moves | occupants | bed            |
      | 11-14   | 0        |       | 1      |               | 1         | occupied, seen |
      | 15      | 0        |       | 1      | yes           | 1         | occupied, seen |
      | 16-20   | 0        |       | 1      |               | 1         | occupied, seen |
    Then between samples 11 and 20 the pose was requested only in 15
    And in sample 20 the pose is stable since sample 15

  Scenario: The pose is looked for with a margin around the subject, and the motion is still measured on their box
    Given the pose crop grows the subject's box by 0.2
    And the resident came in alone and lay down in samples 1 to 10
    When the camera sees this sequence:
      | samples | on floor | where | in bed | subject moves | occupants | bed            |
      | 11-14   | 0        |       | 1      |               | 1         | occupied, seen |
      | 15      | 0        |       | 1      | yes           | 1         | occupied, seen |
      | 16-20   | 0        |       | 1      |               | 1         | occupied, seen |
    Then in sample 15 the pose was requested over the subject's box grown by 0.2
    And between samples 11 and 20 the pose was requested only in 15

  Scenario: Held: the pose is not requested
    Given the resident came in alone and lay down in samples 1 to 10
    When the camera sees this sequence:
      | samples | on floor | where | in bed | occupants | bed            |
      | 11-20   | 0        |       | 0      | 1         | occupied, held |
    Then the pose was not requested in samples 11 to 20
    And in sample 20 the pose was not requested because the subject was not seen in this sample

  Scenario: Accompanied: there is no subject and no pose
    Given the resident came in alone and lay down in samples 1 to 10
    When the camera sees this sequence:
      | samples | on floor | where | in bed | occupants | bed            |
      | 11-15   | 1        | door  | 1      | 2         | occupied, seen |
      | 16-20   | 0        |       | 1      | 1         | occupied, seen |
    Then in sample 15 there is no subject
    And the pose was not requested in samples 11 to 15
    And the first pose after sample 15 was requested without a reference

  Scenario: The pose fails: it is erased and requested again, even if they go back to the same place
    Given the resident came in alone and lay down in samples 1 to 10
    When the camera sees this sequence:
      | samples | on floor | where | in bed | subject moves | pose  | occupants | bed            |
      | 11      | 0        |       | 1      | yes           | fails | 1         | occupied, seen |
      | 12      | 0        |       | 1      | yes           |       | 1         | occupied, seen |
    Then in sample 11 there is no pose
    And in sample 12 the pose is new

  Scenario: Seen only by reinforcement: the motion reference does not move
    Given there is a reinforcement detector
    And the resident came in alone and lay down in samples 1 to 10
    When the camera sees this sequence:
      | samples | on floor | where | in bed | reinforcement in bed | subject moves | occupants | bed            |
      | 11      | 0        |       | 0      | 1                    | yes           | 1         | occupied, seen |
      | 12-15   | 0        |       | 1      |                      |               | 1         | occupied, seen |
    Then in sample 11 the looks were "detect, reinforce, pose"
    And in sample 12, seen again by the main detector, the pose was requested

  Scenario: Without a pose model, the engine neither requests nor records it
    Given there is no pose model
    And the resident came in alone and lay down in samples 1 to 10
    When the camera sees this sequence:
      | samples | on floor | where | in bed | occupants | bed            |
      | 11      | 0        |       | 1      | 1         | occupied, seen |
    Then in sample 11 the looks were "detect"
    And in sample 11 there is no pose

  Scenario: A night with pose replays without a camera and gives the same
    Given the resident came in alone and lay down in samples 1 to 10
    When the camera sees this sequence:
      | samples | on floor | where | in bed | subject moves | pose  | occupants | bed            |
      | 11-14   | 0        |       | 1      |               |       | 1         | occupied, seen |
      | 15      | 0        |       | 1      | yes           | fails | 1         | occupied, seen |
      | 16-18   | 0        |       | 0      |               |       | 1         | occupied, held |
      | 19-22   | 0        |       | 1      | yes           |       | 1         | occupied, seen |
    Then the same night, replayed with the same readings, gives the same scenes and events
