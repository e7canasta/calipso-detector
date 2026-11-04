# language: en
Feature: Who was seen stepping down beside the bed left it
  As care staff
  I want a resident who gets out of bed while a carer leaves to have left the bed
  so that the room is not believed to hold her in an empty bed

  # Rule 1, refined, a capability of the profile until it is proven on every
  # camera (ADR 2026-11-04, el paso visto junto a la cama): the bed goes down
  # and, while the count stabilized, one more than at rest was seen on the
  # floor near the bed. If fewer are left on the floor than were seen near it,
  # one of them stepped down from the bed and left: that is decided at once.
  # Otherwise the resident is held, marked as seen stepping: if she reappears
  # in bed she never left; if the floor empties first, she left with whoever
  # left it. galia2, turned, seconds 104-127; galia1, turned, seconds 11-20.

  Background:
    Given a room with the bed delimited
    And stabilization requires 1 second of source time
    And samples arrive every 1 second of source time

  Scenario: Without the rule, the resident who steps down as the carer leaves stays held
    Given the resident came in alone and lay down in samples 1 to 10
    When the camera sees this sequence:
      | samples | on floor | where      | in bed | occupants | bed            |
      | 11-15   | 1        | near bed   | 1      | 2         | occupied, seen |
      | 16      | 2        | near bed   | 0      | 2         | occupied, seen |
      | 17-18   | 1        | near bed   | 0      | 2         | occupied, held |
      | 19-25   | 0        |            | 0      | 1         | occupied, held |
    Then no exit from the bed was observed

  Scenario: With the rule, the resident who steps down as the carer leaves left the bed
    Given whoever is seen stepping down beside the bed left it
    And the resident came in alone and lay down in samples 1 to 10
    When the camera sees this sequence:
      | samples | on floor | where      | in bed | occupants | bed            |
      | 11-15   | 1        | near bed   | 1      | 2         | occupied, seen |
      | 16      | 2        | near bed   | 0      | 2         | occupied, seen |
      | 17-18   | 1        | near bed   | 0      | 2         | occupied, held |
      | 19-25   | 0        |            | 0      | 0         | empty          |
    Then an exit from the bed was observed
    And nobody is held in bed

  Scenario: With the rule, the resident hidden by the carer who shows again never left
    Given whoever is seen stepping down beside the bed left it
    And the resident came in alone and lay down in samples 1 to 10
    When the camera sees this sequence:
      | samples | on floor | where      | in bed | occupants | bed            |
      | 11-15   | 1        | near bed   | 1      | 2         | occupied, seen |
      | 16      | 2        | near bed   | 0      | 2         | occupied, seen |
      | 17-18   | 1        | near bed   | 0      | 2         | occupied, held |
      | 19-22   | 1        | near bed   | 1      | 2         | occupied, seen |
      | 23-30   | 0        |            | 1      | 1         | occupied, seen |
    Then no exit from the bed was observed

  Scenario: With the rule, the resident who gets up and goes within one count left the bed
    Given whoever is seen stepping down beside the bed left it
    And the resident came in alone and lay down in samples 1 to 10
    When the camera sees this sequence:
      | samples | on floor | where      | in bed | occupants | bed            |
      | 11      | 1        | near bed   | 0      | 1         | occupied, seen |
      | 12-20   | 0        |            | 0      | 0         | empty          |
    Then an exit from the bed was observed
    And nobody is held in bed

  Scenario: Without the rule, the resident who gets up and goes within one count is held
    Given the resident came in alone and lay down in samples 1 to 10
    When the camera sees this sequence:
      | samples | on floor | where      | in bed | occupants | bed            |
      | 11      | 1        | near bed   | 0      | 1         | occupied, seen |
      | 12-20   | 0        |            | 0      | 1         | occupied, held |
    Then no exit from the bed was observed

  Scenario: With the rule, one more seen at the door is not a step from the bed
    Given whoever is seen stepping down beside the bed left it
    And the resident came in alone and lay down in samples 1 to 10
    When the camera sees this sequence:
      | samples | on floor | where      | in bed | occupants | bed            |
      | 11-15   | 1        | near bed   | 1      | 2         | occupied, seen |
      | 16      | 2        | 1 near bed | 0      | 2         | occupied, seen |
      | 17-18   | 1        | near bed   | 0      | 2         | occupied, held |
    Then no exit from the bed was observed
