# language: en
Feature: Calibrate the empty bed
  As whoever is responsible for installing the camera
  I want the engine to learn on its own how the empty bed looks, at the first moment it is truly empty
  to have a reference to compare against later, without ever learning a person as if it were the bed

  Background:
    Given a room with the bed delimited
    And stabilization requires 2 seconds of source time
    And samples arrive every 1 second of source time
    And there is a depth model
    And the bed is calibrated with a retry of 10 seconds of source time
    And the bed has no depth reference

  Scenario: The empty room is calibrated once
    When the camera sees this sequence:
      | samples | on floor | where | in bed | occupants | bed   |
      | 1-20    | 0        |       | 0      | 0         | empty |
    Then the bed was calibrated in sample 1
    And calibration was requested in samples 1
    And the scene publishes the calibrated bed

  Scenario: With a depth crop, the empty bed is learned through the same crop as the subject
    Given depth is measured over the bed grown by 0.25
    When the camera sees this sequence:
      | samples | on floor | where | in bed | occupants | bed   |
      | 1-3     | 0        |       | 0      | 0         | empty |
    Then in sample 1 calibration was requested over the bed grown by 0.25

  Scenario: With someone in the room there is no calibration
    Given the resident came in alone and lay down in samples 1 to 10
    When the camera sees this sequence:
      | samples | on floor | where | in bed | occupants | bed            |
      | 11-20   | 0        |       | 1      | 1         | occupied, seen |
    Then calibration was not requested
    And in sample 20 the bed was not calibrated because someone is in the room

  Scenario: With a face in sight there is no calibration
    Given there is a face detector with minimum confidence 0.5
    And the bed's headboard delimited
    When the camera sees this sequence:
      | samples | on floor | where | in bed | faces        | occupants | bed   |
      | 1-5     | 0        |       | 0      | at headboard | 0         | empty |
      | 6-8     | 0        |       | 0      |              | 0         | empty |
    Then in sample 5 the bed was not calibrated because faces are in sight
    And the bed was calibrated in sample 6

  Scenario: A face beside the headboard, found by the margin, also holds the calibration back
    Given there is a face detector with minimum confidence 0.5
    And the bed's headboard delimited
    And faces are looked for at the headboard grown by 0.5
    When the camera sees this sequence:
      | samples | on floor | where | in bed | faces            | occupants | bed   |
      | 1-5     | 0        |       | 0      | beside headboard | 0         | empty |
      | 6-8     | 0        |       | 0      |                  | 0         | empty |
    Then in sample 5 the bed was not calibrated because faces are in sight
    And the bed was calibrated in sample 6

  Scenario: The last one just left: it waits for the count to stabilize
    Given the resident came in alone and lay down in samples 1 to 10
    When the camera sees this sequence:
      | samples | on floor | where    | in bed | occupants | bed   |
      | 11-14   | 1        | near bed | 0      | 1         | empty |
      | 15      | 1        | door     | 0      | 1         | empty |
      | 16-25   | 0        |          | 0      | 0         | empty |
    Then calibration was not requested before the room was left with 0 occupants
    And the bed was calibrated only once

  Scenario: An invalid measurement waits for the retry delay, in source time
    Given samples arrive every 2 seconds of source time
    When the camera sees this sequence:
      | samples | on floor | where | in bed | bed depth | occupants | bed   |
      | 1       | 0        |       | 0      | invalid   | 0         | empty |
      | 2-5     | 0        |       | 0      |           | 0         | empty |
      | 6-7     | 0        |       | 0      |           | 0         | empty |
    Then in sample 5 the bed was not calibrated because the retry delay has not passed yet
    And calibration was requested in samples 1 and 6
    And the bed was calibrated in sample 6

  Scenario: Without bed geometry there is no calibration, and it says why
    Given the camera delivers images of another size than the delimited bed's
    When the camera sees this sequence:
      | samples | on floor | where | in bed | occupants | bed     |
      | 1-3     | 0        |       | 0      | 0         | unknown |
    Then in sample 3 the bed was not calibrated because there is no bed geometry

  Scenario: Adopted at startup: no calibration with them on it
    When the camera sees this sequence:
      | samples | on floor | where | in bed | occupants | bed            |
      | 1-10    | 0        |       | 1      | 1         | occupied, seen |
    Then calibration was not requested

  Scenario: A night with calibration replays without a camera and gives the same
    When the camera sees this sequence:
      | samples | on floor | where | in bed | bed depth | occupants | bed            |
      | 1       | 0        |       | 0      | invalid   | 0         | empty          |
      | 2-12    | 0        |       | 0      |           | 0         | empty          |
      | 13-15   | 1        | door  | 0      |           | 1         | empty          |
      | 16-20   | 0        |       | 1      |           | 1         | occupied, seen |
    Then the same night, replayed with the same readings, gives the same scenes and events
