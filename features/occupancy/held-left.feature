# language: en
Feature: The held resident gets up by the side
  As care staff
  I want a resident lost in bed and seen again beside it to have got up
  so that a resident alone on the edge of the bed is not read as accompanied
  and her alerts are not silenced (white book, volume VII, ch. 05)

  # Rule 4 bis, a capability of the profile until it is proven on every
  # camera: with someone held in bed, a body that appears on the floor near
  # the bed, and not from the door, is the held one who got up.

  Background:
    Given a room with the bed delimited
    And stabilization requires 2 seconds of source time
    And samples arrive every 1 second of source time

  Scenario: Without the side exit, the held resident seen beside the bed is someone else
    Given the resident came in alone and lay down in samples 1 to 10
    When the camera sees this sequence:
      | samples | on floor | where    | in bed | occupants | bed            |
      | 11-20   | 0        |          | 0      | 1         | occupied, held |
      | 21-30   | 1        | near bed | 0      | 2         | occupied, held |
    Then the room is "Several"

  Scenario: With the side exit, the held resident seen beside the bed got up
    Given a held resident can leave the bed by the side
    And the resident came in alone and lay down in samples 1 to 10
    When the camera sees this sequence:
      | samples | on floor | where    | in bed | occupants | bed            |
      | 11-20   | 0        |          | 0      | 1         | occupied, held |
      | 21-30   | 1        | near bed | 0      | 1         | empty          |
    Then the room is "One"
    And an exit from the bed was observed
    And nobody is held in bed

  Scenario: With the side exit, whoever comes from the door is someone else
    Given a held resident can leave the bed by the side
    And the resident came in alone and lay down in samples 1 to 10
    When the camera sees this sequence:
      | samples | on floor | where | in bed | occupants | bed            |
      | 11-20   | 0        |       | 0      | 1         | occupied, held |
      | 21-30   | 1        | door  | 0      | 2         | occupied, held |
    Then the room is "Several"
    And no exit from the bed was observed

  Scenario: With the side exit, the held resident gets up and someone comes from the door
    Given a held resident can leave the bed by the side
    And the resident came in alone and lay down in samples 1 to 10
    When the camera sees this sequence:
      | samples | on floor | where         | in bed | occupants | bed   |
      | 11-20   | 0        |            | 0      | 1         | occupied, held |
      | 21-30   | 2        | 1 near bed | 0      | 2         | empty          |
    Then the room is "Several"
    And an exit from the bed was observed
