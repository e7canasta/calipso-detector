# language: en
Feature: The scene declares when it did not observe
  As a consumer of the scene
  I want to know whether what the scene claims came from an observation of this sample
  so as not to read "seen" for a bed nobody looked at

  Background:
    Given a room with the bed delimited
    And stabilization requires 2 seconds of source time
    And samples arrive every 1 second of source time

  Scenario: With the detector down, the scene keeps what is believed and declares it
    Given the resident came in alone and lay down in samples 1 to 10
    And the detector fails in samples 11 to 15
    Then the room is "One"
    And the scene declares no new observation since sample 10
    And no occupancy event was emitted after sample 10

  Scenario: When the detector comes back, the scene is an observation again
    Given the resident came in alone and lay down in samples 1 to 10
    And the detector fails in samples 11 to 12
    When the camera sees this sequence:
      | samples | on floor | where | in bed | occupants | bed            |
      | 13-15   | 0        |       | 1      | 1         | occupied, seen |
    Then the scene is an observation of the last sample

  Scenario: A decode failure after a detector failure keeps the last real observation
    Given the resident came in alone and lay down in samples 1 to 10
    And the detector fails in samples 11 to 11
    When sample 12 cannot be decoded
    Then the republished scene keeps the last observation at sample 10

  Scenario: A sensor change after a detector failure keeps the last real observation
    Given the resident came in alone and lay down in samples 1 to 10
    And the detector fails in samples 11 to 11
    When the sensor stalls at sample 12
    Then the republished scene keeps the last observation at sample 10

  Scenario: Chained failures at startup do not invent a first observation
    Given the detector fails in samples 1 to 1
    When sample 2 cannot be decoded
    Then the republished scene declares there were no observations yet

  Scenario: A reading of another kind from the detector is a failing sense: the scene declares it did not observe
    Given the resident came in alone and lay down in samples 1 to 10
    And the detector answers another kind of reading in samples 11 to 15
    Then in sample 11 the detector look failed
    And the room is "One"
    And the scene declares no new observation since sample 10
    And no occupancy event was emitted after sample 10
