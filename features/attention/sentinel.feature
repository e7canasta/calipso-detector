# language: en
Feature: The sentinel: the whole frame now and then
  As care staff
  I want the engine, even while it looks at the bed up close, to look at the whole room now and then
  so as to learn of whoever comes in far from the bed, even if a little late

  # Lying down is confirmed in sample 8: that turn still looked at the whole
  # frame, so the sentinel is due 30 seconds later, in sample 38. Every
  # whole-frame look that read counts: the sentinel's, a widening's or one
  # while a change was stabilizing.

  Background:
    Given a room with the bed delimited
    And stabilization requires 2 seconds of source time
    And samples arrive every 1 second of source time
    And the main detector focuses on the bed grown by 0.25 and looks at the whole frame every 30 seconds of source time

  Scenario: Still in bed: the whole frame every 30 seconds
    Given the resident came in alone and lay down in samples 1 to 10
    When the camera sees this sequence:
      | samples | on floor | where | in bed | occupants | bed            |
      | 11-75   | 0        |       | 1      | 1         | occupied, seen |
    Then between samples 11 and 75 the main detector looked at the whole frame only in samples 38 and 68

  Scenario: Someone comes in at the door: seen at the sentinel, not before
    Given the resident came in alone and lay down in samples 1 to 10
    When the camera sees this sequence:
      | samples | on floor | where | in bed | occupants | bed            |
      | 11-30   | 0        |       | 1      | 1         | occupied, seen |
      | 31-45   | 1        | door  | 1      | 2         | occupied, seen |
    Then the room had 1 occupant from sample 11 to sample 38
    And the room is "Several"

  Scenario: A widening counts as a whole-frame look
    # Sample 20 widens; sample 21 still looks whole while the loss stabilizes.
    Given the resident came in alone and lay down in samples 1 to 10
    When the camera sees this sequence:
      | samples | on floor | where | in bed | occupants | bed            |
      | 11-19   | 0        |       | 1      | 1         | occupied, seen |
      | 20      | 0        |       | 0      | 1         | occupied, seen |
      | 21-55   | 0        |       | 1      | 1         | occupied, seen |
    Then between samples 22 and 55 the main detector looked at the whole frame only in sample 51

  Scenario: A camera reconnection: the first turn looks at the whole frame
    Given the resident came in alone and lay down in samples 1 to 10
    And the camera reconnects before sample 21
    When the camera sees this sequence:
      | samples | on floor | where | in bed | occupants | bed            |
      | 11-25   | 0        |       | 1      | 1         | occupied, seen |
    Then in sample 21 the main detector looked at the whole frame
    And in sample 22 the main detector looked at the bed grown by 0.25
