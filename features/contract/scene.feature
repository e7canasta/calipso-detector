# language: en
Feature: The scene says what is seen and what is believed
  As a consumer of the scene
  I want to tell the seen bed from the held bed, and what is seen from what is believed
  to decide without reimplementing the model

  Background:
    Given a room with the bed delimited
    And stabilization requires 2 seconds of source time
    And samples arrive every 1 second of source time

  Scenario: The seen bed and the held bed are different states
    Given the resident came in alone and lay down in samples 1 to 10
    When the camera sees this sequence:
      | samples | on floor | where | in bed | occupants | bed            |
      | 11-20   | 0        |       | 0      | 1         | occupied, held |
      | 21-25   | 0        |       | 1      | 1         | occupied, seen |
    Then the bed was observed going from "seen" to "held"
    And the bed was observed going from "held" to "seen"
    And the number of people in the room did not change after sample 10

  Scenario: Each event carries how the room was left
    Given the resident came in alone and lay down in samples 1 to 10
    When the camera sees this sequence:
      | samples | on floor | where | in bed | occupants | bed            |
      | 11-20   | 0        |       | 0      | 1         | occupied, held |
    Then the "held_in_bed" event left the bed "held" with 0 seen and 1 held

  Scenario: A flicker shows in the evidence and not in the belief
    Given the resident came in alone and lay down in samples 1 to 10
    When the camera sees this sequence:
      | samples | on floor | where | in bed | occupants | bed            |
      | 11      | 0        |       | 0      | 1         | occupied, seen |
    Then the evidence of the last sample has no bodies
    And there is a pending change at 0 of 2 seconds

  Scenario: The held bed says since when
    Given the resident came in alone and lay down in samples 1 to 10
    When the camera sees this sequence:
      | samples | on floor | where | in bed | occupants | bed            |
      | 11-30   | 0        |       | 0      | 1         | occupied, held |
    Then the bed is "held" since sample 13
    And the held occupant was last seen in sample 10

  Scenario: S7: someone enters while the resident is covered
    When the camera sees this sequence:
      | samples | on floor | where    | in bed | occupants | bed            |
      | 1-5     | 1        | near bed | 1      | 2         | occupied, seen |
      | 6-10    | 1        | near bed | 0      | 2         | occupied, held |
      | 11-15   | 2        | door     | 0      | 3         | occupied, held |
    Then the room is "Several"
    And no exit from the bed was observed
