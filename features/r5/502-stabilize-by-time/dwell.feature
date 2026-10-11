# language: en
Feature: Stabilize by time
  As whoever installs a camera
  I want a change to count once it has held for a source time, not a number of samples
  so the same policy behaves the same with any camera and any cadence

  Background:
    Given a room with the bed delimited
    And stabilization requires 6 seconds of source time

  Scenario Outline: The entry counts at 6 seconds, with a sample every <cadence> seconds
    Given samples arrive every <cadence> seconds of source time
    When a person appears at the door and stays for <samples> samples
    Then an entry into the room was observed 6 seconds of source time after first seeing them

    Examples:
      | cadence | samples |
      | 1       | 10      |
      | 2       | 5       |
      | 3       | 4       |

  Scenario: While stabilizing, the scene says how much time is left
    Given samples arrive every 1 second of source time
    When the camera sees this sequence:
      | samples | on floor | where | in bed | occupants | bed   |
      | 1-3     | 1        | door  | 0      | 0         | empty |
    Then there is a pending change at 2 of 6 seconds

  Scenario: An interrupted change starts counting from zero again
    Given samples arrive every 1 second of source time
    When the camera sees this sequence:
      | samples | on floor | where | in bed | occupants | bed   |
      | 1-5     | 1        | door  | 0      | 0         | empty |
      | 6       | 0        |       | 0      | 0         | empty |
      | 7-10    | 1        | door  | 0      | 0         | empty |
    Then there is a pending change at 3 of 6 seconds
    And no entry into the room was observed
