# language: en
Feature: The main detector focuses on the bed
  As care staff
  I want the engine, while the resident is alone in bed, to look at the bed up close
  so as not to lose them to a blanket or a sideways posture, which from afar are few pixels

  # The scenes' bed is 100..500 x 100..400. Grown by 0.25 it is 0..600 x 25..475:
  # a body "near bed" falls inside the focus, one at the "door" does not.
  # Where the main detector looked is the region of its look in the ledger
  # (none: the whole frame); the published contract does not change.

  Background:
    Given a room with the bed delimited
    And stabilization requires 2 seconds of source time
    And samples arrive every 1 second of source time
    And the main detector focuses on the bed grown by 0.25 and looks at the whole frame every 30 seconds of source time

  Scenario: Alone and still in bed: the main detector looks at the focus
    Given the resident came in alone and lay down in samples 1 to 10
    When the camera sees this sequence:
      | samples | on floor | where | in bed | occupants | bed            |
      | 11-15   | 0        |       | 1      | 1         | occupied, seen |
    Then in sample 12 the looks were "detect"
    And in sample 12 the main detector looked at the bed grown by 0.25

  Scenario: The focus is grown by its own padding, not by reinforcement's
    Given there is a reinforcement detector
    And the resident came in alone and lay down in samples 1 to 10
    When the camera sees this sequence:
      | samples | on floor | where | in bed | occupants | bed            |
      | 11-15   | 0        |       | 1      | 1         | occupied, seen |
    Then in sample 12 the main detector looked at the bed grown by 0.25

  Scenario: The site map's focus wins over the grown bed
    Given the site map delimits the focus as 50,50 to 590,460
    And the resident came in alone and lay down in samples 1 to 10
    When the camera sees this sequence:
      | samples | on floor | where | in bed | occupants | bed            |
      | 11-15   | 0        |       | 1      | 1         | occupied, seen |
    Then in sample 12 the main detector looked at 50,50 to 590,460

  Scenario: Held in bed is also focused
    Given the resident came in alone and lay down in samples 1 to 10
    When the camera sees this sequence:
      | samples | on floor | where | in bed | occupants | bed            |
      | 11-20   | 0        |       | 0      | 1         | occupied, held |
    Then in sample 20 the looks were "detect"
    And in sample 20 the main detector looked at the bed grown by 0.25

  Scenario: While a change is stabilizing, the whole frame is looked at
    Given the resident came in alone and lay down in samples 1 to 10
    When the camera sees this sequence:
      | samples | on floor | where | in bed | occupants | bed            |
      | 11-20   | 0        |       | 0      | 1         | occupied, held |
    Then in sample 12 the main detector looked at the whole frame

  Scenario: Accompanied, the whole frame is looked at
    Given the staff put the resident to bed and left in samples 1 to 25
    When the camera sees this sequence:
      | samples | on floor | where    | in bed | occupants | bed            |
      | 26-35   | 1        | near bed | 1      | 2         | occupied, seen |
    Then in sample 35 the main detector looked at the whole frame

  Scenario: Empty room or someone only on the floor: the whole frame is looked at
    When the camera sees this sequence:
      | samples | on floor | where | in bed | occupants | bed   |
      | 1-5     | 0        |       | 0      | 0         | empty |
      | 6-10    | 1        | door  | 0      | 1         | empty |
    Then in sample 5 the main detector looked at the whole frame
    And in sample 10 the main detector looked at the whole frame

  Scenario: Without bed geometry there is no focus
    Given the camera delivers images of another size than the delimited bed's
    When the camera sees this sequence:
      | samples | on floor | where | in bed | occupants | bed     |
      | 1-3     | 0        |       | 0      | 0         | unknown |
    Then in sample 3 the main detector looked at the whole frame

  Scenario: Without the focus policy, the engine does not focus
    Given the main detector does not focus
    And the resident came in alone and lay down in samples 1 to 10
    When the camera sees this sequence:
      | samples | on floor | where | in bed | occupants | bed            |
      | 11-15   | 0        |       | 1      | 1         | occupied, seen |
    Then in sample 12 the main detector looked at the whole frame
    And in sample 12 the looks were "detect"

  Scenario: A night with the focus replays without a camera and gives the same
    Given the resident came in alone and lay down in samples 1 to 10
    When the camera sees this sequence:
      | samples | on floor | where    | in bed | occupants | bed            |
      | 11-20   | 0        |          | 1      | 1         | occupied, seen |
      | 21-30   | 0        |          | 0      | 1         | occupied, held |
      | 31-35   | 0        |          | 1      | 1         | occupied, seen |
      | 36-45   | 1        | near bed | 0      | 1         | empty          |
    Then the same night, replayed with the same readings, gives the same scenes and events

  Scenario: The scene publishes the focus with the rest of the map
    When the camera sees this sequence:
      | samples | on floor | where | in bed | occupants | bed   |
      | 1-3     | 0        |       | 0      | 0         | empty |
    Then the scene's map carries the bed grown by 0.25 as the focus

  Scenario: The site map's focus is the one published
    Given the site map delimits the focus as 50,50 to 590,460
    When the camera sees this sequence:
      | samples | on floor | where | in bed | occupants | bed   |
      | 1-3     | 0        |       | 0      | 0         | empty |
    Then the scene's map carries 50,50 to 590,460 as the focus

  Scenario: Without the focus policy, the map carries no focus
    Given the main detector does not focus
    When the camera sees this sequence:
      | samples | on floor | where | in bed | occupants | bed   |
      | 1-3     | 0        |       | 0      | 0         | empty |
    Then the scene's map carries no focus
