# language: en
Feature: Failing reinforcement does not spend the loss
  As care staff
  I want the finer detector, if it fails looking at the bed, to try again on the next sample
  so a loss is not left unchecked because of an instrument failure

  Background:
    Given a room with the bed delimited
    And stabilization requires 2 seconds of source time
    And samples arrive every 1 second of source time
    And there is a reinforcement detector

  Scenario: While the main detector sees someone in bed, there is no loss to reinforce
    Given the resident came in alone and lay down in samples 1 to 10
    Then reinforcement was not requested

  Scenario: If reinforcement fails, it is requested again on the next sample
    Given the resident came in alone and lay down in samples 1 to 10
    And reinforcement fails
    When the camera sees this sequence:
      | samples | on floor | where | in bed | occupants | bed            |
      | 11-12   | 0        |       | 0      | 1         | occupied, seen |
    Then reinforcement was requested in samples 11 and 12

  Scenario: If reinforcement looked, the loss is checked only once
    Given the resident came in alone and lay down in samples 1 to 10
    When the camera sees this sequence:
      | samples | on floor | where | in bed | occupants | bed            |
      | 11-14   | 0        |       | 0      | 1         | occupied, held |
    Then reinforcement was requested only in sample 11
