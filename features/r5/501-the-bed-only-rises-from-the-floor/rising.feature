# language: en
Feature: The bed only rises from the floor
  As care staff
  I want nobody to appear in bed without having been seen in the room before
  so an object the detector mistakes for a person does not occupy the bed

  Background:
    Given a room with the bed delimited
    And stabilization requires 2 seconds of source time
    And samples arrive every 1 second of source time

  Scenario: A body in the empty bed, with nobody coming from the floor, is not an occupant
    Given the resident came in alone and lay down in samples 1 to 10
    When the camera sees this sequence:
      | samples | on floor | where    | in bed | occupants | bed   |
      | 11-15   | 1        | near bed | 0      | 1         | empty |
      | 16-25   | 0        |          | 0      | 0         | empty |
      | 26-35   | 0        |          | 1      | 0         | empty |
    Then the room is "Empty"
    And the bed has 1 unexplained body
    And nobody was observed lying down after sample 25

  Scenario: What was nobody leaves nobody held when it disappears
    Given the resident came in alone and lay down in samples 1 to 10
    When the camera sees this sequence:
      | samples | on floor | where    | in bed | occupants | bed   |
      | 11-15   | 1        | near bed | 0      | 1         | empty |
      | 16-25   | 0        |          | 0      | 0         | empty |
      | 26-35   | 0        |          | 1      | 0         | empty |
      | 36-45   | 0        |          | 0      | 0         | empty |
    Then the bed has 0 unexplained bodies
    And nobody is held in bed

  Scenario: A body in bed while the staff stay on the floor is not an occupant
    Given the resident came in alone and lay down in samples 1 to 10
    When the camera sees this sequence:
      | samples | on floor | where    | in bed | occupants | bed   |
      | 11-15   | 1        | near bed | 0      | 1         | empty |
      | 16-25   | 0        |          | 0      | 0         | empty |
      | 26-30   | 1        | door     | 0      | 1         | empty |
      | 31-40   | 1        | door     | 1      | 1         | empty |
    Then the room is "One"
    And the bed has 1 unexplained body

  Scenario: Enters and lies down at once, before the floor stabilizes
    Given the resident came in alone and lay down in samples 1 to 10
    When the camera sees this sequence:
      | samples | on floor | where    | in bed | occupants | bed            |
      | 11-15   | 1        | near bed | 0      | 1         | empty          |
      | 16-25   | 0        |          | 0      | 0         | empty          |
      | 26-27   | 1        | door     | 0      | 0         | empty          |
      | 28-35   | 0        |          | 1      | 1         | occupied, seen |
    Then someone was observed lying down after sample 25
    And the bed has 0 unexplained bodies

  Scenario: At startup, whoever is clearly seen in bed is adopted
    When the camera sees this sequence:
      | samples | on floor | where | in bed | occupants | bed            |
      | 1-5     | 0        |       | 1      | 1         | occupied, seen |
    Then the bed was adopted at startup

  Scenario: At startup a bed in which nobody is seen is not taken as occupied
    When the camera sees this sequence:
      | samples | on floor | where | in bed | occupants | bed   |
      | 1-10    | 0        |       | 0      | 0         | empty |
    Then the room is "Empty"
    And nobody is held in bed

  Scenario: An unstabilized arrival from another session does not explain a body in bed
    Given the camera reconnects before sample 27
    And the resident came in alone and lay down in samples 1 to 10
    When the camera sees this sequence:
      | samples | on floor | where    | in bed | occupants | bed   |
      | 11-15   | 1        | near bed | 0      | 1         | empty |
      | 16-25   | 0        |          | 0      | 0         | empty |
      | 26      | 1        | door     | 0      | 0         | empty |
      | 27-30   | 0        |          | 1      | 0         | empty |
    Then the bed has 1 unexplained body
    And nobody was observed lying down after sample 25

  Scenario: An arrival from the door in another session does not prevent observing an exit from bed
    Given the camera reconnects before sample 5
    When the camera sees this sequence:
      | samples | on floor | where    | in bed | occupants | bed            |
      | 1-3     | 0        |          | 1      | 1         | occupied, seen |
      | 4       | 1        | door     | 0      | 1         | occupied, seen |
      | 5-7     | 1        | near bed | 0      | 1         | empty          |
    Then an exit from the bed was observed
    And nobody is held in bed
