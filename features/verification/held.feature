# language: en
Feature: The held occupant is verified, never released by it
  As care staff
  I want a resident held in bed, unseen, to be looked at again by a slower verifier
  so as to know whether the belief has an observation behind it, without the
  verifier ever moving a count

  Background:
    Given a room with the bed delimited
    And stabilization requires 2 seconds of source time
    And samples arrive every 1 second of source time
    And held occupants are verified after 30 seconds
    And the verifier answers after 3 seconds of source time

  Scenario: A held occupant is confirmed by the verifier
    Given the resident came in alone and lay down in samples 1 to 10
    And the verifier answers "yes"
    When the camera sees this sequence:
      | samples | on floor | where | in bed | occupants | bed            |
      | 11-50   | 0        |       | 0      | 1         | occupied, held |
    Then in sample 42 the held occupant is "unverified"
    And in sample 43 a verification of the held occupant is asked
    And in sample 45 the held occupant is "unverified"
    And in sample 46 the held occupant is "confirmed" by "vlm", looked in sample 43
    And the room is "One"
    And no exit from the bed was observed
