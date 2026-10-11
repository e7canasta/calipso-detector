# language: en
Feature: The bed retains
  As care staff
  I want the bed to stay occupied while the resident sleeps covered
  and become empty only when they were seen leaving
  so as not to get false empty-bed alarms, nor miss a real exit

  Background:
    Given a room with the bed delimited
    And stabilization requires 2 seconds of source time
    And samples arrive every 1 second of source time

  Scenario: Sleeps covered: the flicker does not empty the bed
    Given the resident came in alone and lay down in samples 1 to 10
    When the camera sees this sequence:
      | samples | on floor | where | in bed  | occupants | bed            |
      | 11-30   | 0        |       | 0 and 1 | 1         | occupied       |
      | 31-40   | 0        |       | 0       | 1         | occupied, held |
    Then the room is "One"
    And no exit from the bed was observed

  Scenario: Covered for an hour, still held
    Given the resident came in alone and lay down in samples 1 to 10
    When the camera sees this sequence:
      | samples | on floor | where | in bed | occupants | bed            |
      | 11-3610 | 0        |       | 0      | 1         | occupied, held |
    Then the bed occupant is held since sample 10
    And no exit from the bed was observed

  Scenario: A camera reconnection does not release the held occupant
    Given the resident came in alone and lay down in samples 1 to 10
    And the camera reconnects before sample 21
    When the camera sees this sequence:
      | samples | on floor | where | in bed | occupants | bed            |
      | 11-20   | 0        |       | 0      | 1         | occupied, held |
      | 21-40   | 0        |       | 0      | 1         | occupied, held |
    Then the bed occupant is held since sample 10
    And no occupancy event was emitted after sample 13

  Scenario: Reappears and is not a new one
    Given the resident came in alone and lay down in samples 1 to 10
    When the camera sees this sequence:
      | samples | on floor | where | in bed | occupants | bed            |
      | 11-20   | 0        |       | 0      | 1         | occupied, held |
      | 21-25   | 0        |       | 1      | 1         | occupied, seen |
    Then the bed occupant reappearing was observed
    And nobody was observed lying down after sample 10

  Scenario: Gets up alone and leaves
    Given the resident came in alone and lay down in samples 1 to 10
    When the camera sees this sequence:
      | samples | on floor | where    | in bed | occupants | bed   |
      | 11-15   | 1        | near bed | 0      | 1         | empty |
      | 16-25   | 0        |          | 0      | 0         | empty |
    Then the room is "Empty"
    And an exit from the bed was observed
    And an exit from the room was observed

  Scenario: Comes back alone, lies down and covers up
    When the camera sees this sequence:
      | samples | on floor | where | in bed | occupants | bed            |
      | 1-5     | 0        |       | 0      | 0         | empty          |
      | 6-10    | 1        | door  | 0      | 1         | empty          |
      | 11-15   | 0        |       | 1      | 1         | occupied, seen |
      | 16-25   | 0        |       | 0      | 1         | occupied, held |
    Then the room is "One"
    And there is a subject, in bed

  Scenario: A held occupant does not leave without reappearing
    Given the resident came in alone and lay down in samples 1 to 10
    When the camera sees this sequence:
      | samples | on floor | where    | in bed | occupants | bed            |
      | 11-15   | 0        |          | 0      | 1         | occupied, held |
      | 16-20   | 1        | near bed | 0      | 2         | occupied, held |
    Then the room is "Several"
    And no exit from the bed was observed
