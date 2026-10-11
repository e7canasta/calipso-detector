# language: en
Feature: Accompanied
  As care staff
  I want the system to understand when staff come in to put to bed, fetch or tend to the resident
  so the bed does not become empty when they leave them covered, nor occupied when they took them away

  Background:
    Given a room with the bed delimited
    And stabilization requires 2 seconds of source time
    And samples arrive every 1 second of source time

  Scenario: The staff put the resident to bed and leave
    When the camera sees this sequence:
      | samples | on floor | where    | in bed | occupants | bed            |
      | 1-5     | 2        | door     | 0      | 2         | empty          |
      | 6-10    | 1        | near bed | 1      | 2         | occupied, seen |
      | 11-15   | 1        | near bed | 0      | 2         | occupied, held |
      | 16-25   | 0        |          | 0      | 1         | occupied, held |
    Then the room is "One"
    And no exit from the bed was observed

  Scenario: They come for them
    Given the staff put the resident to bed and left in samples 1 to 25
    When the camera sees this sequence:
      | samples | on floor | where    | in bed | occupants | bed            |
      | 26-30   | 1        | door     | 0      | 2         | occupied, held |
      | 31-35   | 1        | near bed | 1      | 2         | occupied, seen |
    Then the room is "Several"
    And the bed occupant reappearing was observed

  Scenario: Taken away
    Given the staff put the resident to bed and left in samples 1 to 25
    When the camera sees this sequence:
      | samples | on floor | where    | in bed | occupants | bed            |
      | 26-30   | 1        | door     | 0      | 2         | occupied, held |
      | 31-35   | 1        | near bed | 1      | 2         | occupied, seen |
      | 36-40   | 2        | near bed | 0      | 2         | empty          |
      | 41-45   | 1        | door     | 0      | 1         | empty          |
      | 46-55   | 0        |          | 0      | 0         | empty          |
    Then the room is "Empty"
    And an exit from the bed was observed

  Scenario: The carer comes in and covers them at once
    Given the resident came in alone and lay down in samples 1 to 10
    When the camera sees this sequence:
      | samples | on floor | where    | in bed | occupants | bed            |
      | 11      | 1        | door     | 1      | 1         | occupied, seen |
      | 12-20   | 1        | near bed | 0      | 2         | occupied, held |
    Then the room is "Several"
    And no exit from the bed was observed
