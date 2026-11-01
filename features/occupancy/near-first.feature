# language: en
Feature: Who appeared near the bed did not come from the door
  As care staff
  I want a resident who gets up and walks off at once to have left the bed
  so that a camera that sees little floor does not keep her held in an empty
  bed (white book, volume VII, ch. 17)

  # Rule 5, refined, a capability of the profile until it is proven on every
  # camera: whoever appeared far from the bed comes from the door; whoever
  # appeared near it and then walked away got up and is heading for it.

  Background:
    Given a room with the bed delimited
    And stabilization requires 1 second of source time
    And samples arrive every 1 second of source time

  Scenario: Without the rule, the resident who gets up and walks off at once is a visitor
    Given the resident came in alone and lay down in samples 1 to 10
    When the camera sees this sequence:
      | samples | on floor | where    | in bed | occupants | bed            |
      | 11      | 1        | near bed | 0      | 1         | occupied, seen |
      | 12      | 1        | door     | 0      | 2         | occupied, held |
      | 13-20   | 0        |          | 0      | 1         | occupied, held |
    Then no exit from the bed was observed

  Scenario: With the rule, the resident who gets up and walks off at once left the bed
    Given whoever appears near the bed did not come from the door
    And the resident came in alone and lay down in samples 1 to 10
    When the camera sees this sequence:
      | samples | on floor | where    | in bed | occupants | bed            |
      | 11      | 1        | near bed | 0      | 1         | occupied, seen |
      | 12      | 1        | door     | 0      | 1         | empty          |
      | 13-20   | 0        |          | 0      | 0         | empty          |
    Then an exit from the bed was observed
    And nobody is held in bed

  Scenario: With the rule, the carer who appears at the door and comes close still came from the door
    Given whoever appears near the bed did not come from the door
    And the resident came in alone and lay down in samples 1 to 10
    When the camera sees this sequence:
      | samples | on floor | where    | in bed | occupants | bed            |
      | 11      | 1        | door     | 1      | 1         | occupied, seen |
      | 12-20   | 1        | near bed | 0      | 2         | occupied, held |
    Then the room is "Several"
    And no exit from the bed was observed
