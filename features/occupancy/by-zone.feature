# language: en
Feature: Occupancy by zone
  As care staff
  I want to know how many people are in the room and how many in bed
  to know whether the resident is alone, accompanied or nobody is there

  Background:
    Given a room with the bed delimited
    And stabilization requires 2 seconds of source time
    And samples arrive every 1 second of source time

  Scenario: Empty room
    When the camera sees this sequence:
      | samples | on floor | where | in bed | occupants | bed   |
      | 1-5     | 0        |       | 0      | 0         | empty |
    Then the room is "Empty"
    And there is no subject

  Scenario: A person enters and stays on the floor
    When the camera sees this sequence:
      | samples | on floor | where | in bed | occupants | bed   |
      | 1-5     | 0        |       | 0      | 0         | empty |
      | 6-7     | 1        | door  | 0      | 0         | empty |
      | 8-10    | 1        | door  | 0      | 1         | empty |
    Then the room is "One"
    And there is a subject, off the bed
    And an entry into the room was observed

  Scenario: Enters and leaves without lying down
    When the camera sees this sequence:
      | samples | on floor | where | in bed | occupants | bed   |
      | 1-5     | 1        | door  | 0      | 1         | empty |
      | 6-10    | 0        |       | 0      | 0         | empty |
    Then the room is "Empty"
    And an exit from the room was observed

  Scenario: Enters and lies down
    When the camera sees this sequence:
      | samples | on floor | where | in bed | occupants | bed            |
      | 1-5     | 1        | door  | 0      | 1         | empty          |
      | 6-10    | 0        |       | 1      | 1         | occupied, seen |
    Then the room is "One"
    And there is a subject, in bed
    And someone lying down was observed

  Scenario: A flicker changes nothing
    When the camera sees this sequence:
      | samples | on floor | where | in bed | occupants | bed            |
      | 1-5     | 0        |       | 1      | 1         | occupied, seen |
      | 6       | 0        |       | 0      | 1         | occupied, seen |
      | 7       | 1        | door  | 1      | 1         | occupied, seen |
      | 8-10    | 0        |       | 1      | 1         | occupied, seen |
    Then no occupancy event was emitted after sample 5

  Scenario: Two people in the room
    When the camera sees this sequence:
      | samples | on floor | where | in bed | occupants | bed   |
      | 1-5     | 2        | door  | 0      | 2         | empty |
    Then the room is "Several"
    And there is no subject
