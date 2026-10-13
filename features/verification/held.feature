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

  Scenario: A "no" does not release the held occupant
    Given the resident came in alone and lay down in samples 1 to 10
    And the verifier answers "no"
    When the camera sees this sequence:
      | samples | on floor | where | in bed | occupants | bed            |
      | 11-60   | 0        |       | 0      | 1         | occupied, held |
    Then in sample 46 the held occupant is "not_confirmed" by "vlm", looked in sample 43
    And the room is "One"
    And no exit from the bed was observed

  Scenario: An answer about an occupant who reappeared does not confirm the next one held
    Given the resident came in alone and lay down in samples 1 to 10
    And the verifier answers "yes"
    When the camera sees this sequence:
      | samples | on floor | where | in bed | occupants | bed            |
      | 11-43   | 0        |       | 0      | 1         | occupied, held |
      | 44-48   | 0        |       | 1      | 1         | occupied, seen |
      | 49-60   | 0        |       | 0      | 1         | occupied, held |
    Then in sample 60 the held occupant is "unverified"

  Scenario: An answer from before a reconnection is discarded, and it is asked again
    Given the camera reconnects before sample 45
    And the resident came in alone and lay down in samples 1 to 10
    And the verifier answers "yes"
    When the camera sees this sequence:
      | samples | on floor | where | in bed | occupants | bed            |
      | 11-80   | 0        |       | 0      | 1         | occupied, held |
    Then in sample 45 the held occupant is "unverified"
    And in sample 75 a verification of the held occupant is asked

  Scenario: The verifier never answers: the request expires and is asked again
    Given verification requests expire after 8 seconds
    And the verifier never answers
    And the resident came in alone and lay down in samples 1 to 10
    When the camera sees this sequence:
      | samples | on floor | where | in bed | occupants | bed            |
      | 11-60   | 0        |       | 0      | 1         | occupied, held |
    Then in sample 43 a verification of the held occupant is asked
    And in sample 50 the verifier is busy
    And in sample 51 a verification of the held occupant is asked
    And the room is "One"

  Scenario: With the budget spent, it does not ask and says why
    Given verification requests expire after 8 seconds
    And the verifier may be asked 2 times per hour
    And the verifier never answers
    And the resident came in alone and lay down in samples 1 to 10
    When the camera sees this sequence:
      | samples | on floor | where | in bed | occupants | bed            |
      | 11-70   | 0        |       | 0      | 1         | occupied, held |
    Then in sample 51 a verification of the held occupant is asked
    And in sample 59 the verifier's budget is spent

  Scenario: A confirmation expires and the held occupant is asked about again
    Given a held occupant's check holds 60 seconds
    And the resident came in alone and lay down in samples 1 to 10
    And the verifier answers "yes"
    When the camera sees this sequence:
      | samples | on floor | where | in bed | occupants | bed            |
      | 11-110  | 0        |       | 0      | 1         | occupied, held |
    Then in sample 46 the held occupant is "confirmed" by "vlm", looked in sample 43
    And in sample 103 a verification of the held occupant is asked
    And in sample 106 the held occupant is "confirmed" by "vlm", looked in sample 103
