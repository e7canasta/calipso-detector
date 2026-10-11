# language: en
Feature: The reinforcement policy
  As care staff
  I want the engine, if the main detector stops seeing whoever was in bed, to look at the bed again with a finer detector
  so as not to lose them to a blanket, without spending that effort on every sample

  Background:
    Given a room with the bed delimited
    And stabilization requires 2 seconds of source time
    And samples arrive every 1 second of source time
    And there is a reinforcement detector

  Scenario: Covers up: reinforcement is requested over the grown bed, after the main detector
    Given the resident came in alone and lay down in samples 1 to 10
    When the camera sees this sequence:
      | samples | on floor | where | in bed | reinforcement in bed | occupants | bed            |
      | 11      | 0        |       | 0      | 1                    | 1         | occupied, seen |
    Then in sample 11 the looks were "detect, reinforce"
    And in sample 11 reinforcement was requested over the grown bed
    And in sample 11 the subject was seen in that sample

  Scenario: What reinforcement finds holds the bed count, only in the sample it looked at
    Given the resident came in alone and lay down in samples 1 to 10
    When the camera sees this sequence:
      | samples | on floor | where | in bed | reinforcement in bed | occupants | bed            |
      | 11      | 0        |       | 0      | 1                    | 1         | occupied, seen |
      | 12-15   | 0        |       | 0      | 1                    | 1         | occupied, held |
    Then reinforcement was requested only in sample 11
    And no exit from the bed was observed

  Scenario: The main detector sees them again and re-arms reinforcement
    Given the resident came in alone and lay down in samples 1 to 10
    When the camera sees this sequence:
      | samples | on floor | where | in bed | reinforcement in bed | occupants | bed            |
      | 11      | 0        |       | 0      | 0                    | 1         | occupied, seen |
      | 12      | 0        |       | 1      | 0                    | 1         | occupied, seen |
      | 13      | 0        |       | 0      | 0                    | 1         | occupied, seen |
    Then reinforcement was requested in samples 11 and 13

  Scenario: A held occupant is not looked at again
    Given the resident came in alone and lay down in samples 1 to 10
    When the camera sees this sequence:
      | samples | on floor | where | in bed | reinforcement in bed | occupants | bed            |
      | 11-60   | 0        |       | 0      | 0                    | 1         | occupied, held |
    Then reinforcement was requested only in sample 11
    And in sample 60 reinforcement was not requested because nobody was in bed

  Scenario: With nobody in bed before, there is no reinforcement
    When the camera sees this sequence:
      | samples | on floor | where | in bed | occupants | bed   |
      | 1-5     | 1        | door  | 0      | 1         | empty |
    Then reinforcement was not requested
    And in sample 5 reinforcement was not requested because nobody was in bed

  Scenario: Without bed geometry there is no reinforcement, and it says why
    Given the camera delivers images of another size than the delimited bed's
    When the camera sees this sequence:
      | samples | on floor | where | in bed | occupants | bed     |
      | 1-3     | 0        |       | 0      | 0         | unknown |
    Then in sample 3 reinforcement was not requested because there is no bed geometry

  Scenario: Without a reinforcement detector, the engine neither requests nor records it
    Given there is no reinforcement detector
    And the resident came in alone and lay down in samples 1 to 10
    When the camera sees this sequence:
      | samples | on floor | where | in bed | occupants | bed            |
      | 11      | 0        |       | 0      | 1         | occupied, seen |
    Then in sample 11 the looks were "detect"

  Scenario: A night with reinforcement replays without a camera and gives the same
    Given the resident came in alone and lay down in samples 1 to 10
    When the camera sees this sequence:
      | samples | on floor | where | in bed | reinforcement in bed | occupants | bed            |
      | 11      | 0        |       | 0      | 1                    | 1         | occupied, seen |
      | 12-15   | 0        |       | 0      | 0                    | 1         | occupied, held |
      | 16-20   | 0        |       | 1      | 0                    | 1         | occupied, seen |
    Then the same night, replayed with the same readings, gives the same scenes and events
