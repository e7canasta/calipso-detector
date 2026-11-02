# language: en
Feature: Contract v5: the same turn, cut at the five changes
  As someone reading a night on the next contract
  I want the scene of v5 to state the same mode as v4, without the changed names
  so that the v5 wire says the same thing as v4, except what v5 changes on purpose

  Background:
    Given a room with the bed delimited
    And stabilization requires 2 seconds of source time
    And samples arrive every 1 second of source time

  Scenario: The v5 scene states the same mode as the v4 scene
    Given the resident came in alone and lay down in samples 1 to 10
    Then the v5 scene of sample 10 says mode "alone_in_bed_seen", phase "running" and last cell "100:01:110"


  Scenario: Change 3: the v5 scene has no verdict, and the v4 scene does
    Given there is a pose model
    And there is a depth model
    And the motion threshold is 0.1 of the subject's diagonal
    Given the resident came in alone and lay down in samples 1 to 10
    When the camera sees this sequence:
      | samples | on floor | where | in bed | subject moves | pose  | occupants | bed            |
      | 11      | 0        |       | 1      | yes           | lying | 1         | occupied, seen |
    Then the v4 scene of sample 11 has a subject verdict
    And the v5 scene of sample 11 has no subject verdict

  Scenario: Change 4: the depths carry their relative names with the same value as v4
    Given there is a pose model
    And there is a depth model
    And the motion threshold is 0.1 of the subject's diagonal
    Given the resident came in alone and lay down in samples 1 to 10
    When the camera sees this sequence:
      | samples | on floor | where | in bed | subject moves | pose  | occupants | bed            |
      | 11      | 0        |       | 1      | yes           | lying | 1         | occupied, seen |
    Then the v5 subject depth of sample 11 is named "relative" and equals the v4 "meters"
    And the v5 bed calibration of sample 11 is named "reference" and equals the v4 "reference_m"

  Scenario: Change 5: a v5 transition names rbh and fv, with the numbers v4 gives
    Given the resident came in alone and lay down in samples 1 to 10
    When the camera sees this sequence:
      | samples | on floor | where    | in bed | occupants | bed            |
      | 11      | 1        | door     | 1      | 1         | occupied, seen |
      | 12-13   | 1        | near bed | 0      | 1         | occupied, seen |
      | 14-15   | 1        | near bed | 0      | 2         | occupied, held |
    Then the v5 transition "110:10:211" names rbh as room, bed and held with the v4 numbers
    And the v5 transition "110:10:211" names fv as floor and bed with the v4 numbers

  Scenario: Change 4: the bed's depth reference is named reference, with the v4 value
    Given there is a depth model
    And the bed is calibrated with a retry of 10 seconds of source time
    And the bed has no depth reference
    When the camera sees this sequence:
      | samples | on floor | where | in bed | occupants | bed   |
      | 1-20    | 0        |       | 0      | 0         | empty |
    Then the bed was calibrated in sample 1
    And the v5 bed calibration of sample 5 is named "reference" and equals the v4 "reference_m"
