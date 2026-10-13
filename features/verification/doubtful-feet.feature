# language: en
Feature: Doubtful feet are filled by the verifier
  As care staff
  I want an ankle that falls in the grey band of the bed's side to be asked
  to the slower verifier, and taken as out if it does not answer in time
  so as to be called when they sit on the edge, where the camera projects
  the hanging legs inside the bed (white book, volume III, ch. 10)

  Background:
    Given a room with the bed delimited
    And the bed's headboard delimited
    And stabilization requires 2 seconds of source time
    And samples arrive every 1 second of source time
    And there is a pose model
    And there is a depth model
    And the motion threshold is 0.1 of the subject's diagonal
    And the bed posture is followed
    And feet within 0.1 of the bed's sides are doubtful
    And the verifier answers after 3 seconds of source time

  Scenario: Feet at the side, doubtful: the verifier says a leg is out: getting out
    Given the resident came in alone and lay down in samples 1 to 10
    And the verifier answers "yes"
    When the camera sees this sequence:
      | samples | on floor | where | in bed | subject moves | pose                | occupants | bed            |
      | 11      | 0        |       | 1      | yes           | lying               | 1         | occupied, seen |
      | 12      | 0        |       | 1      | yes           | sitting on the edge | 1         | occupied, seen |
      | 13-16   | 0        |       | 1      |               |                     | 1         | occupied, seen |
    Then in sample 12 the bed posture is "sitting_up", seen "headboard_upright"
    And in sample 12 a verification of the feet is asked
    And in sample 15 the bed posture is "getting_out", seen "legs_out"

  Scenario: Feet at the side, doubtful: the verifier says no: not getting out
    Given the resident came in alone and lay down in samples 1 to 10
    And the verifier answers "no"
    When the camera sees this sequence:
      | samples | on floor | where | in bed | subject moves | pose                | occupants | bed            |
      | 11      | 0        |       | 1      | yes           | lying               | 1         | occupied, seen |
      | 12      | 0        |       | 1      | yes           | sitting on the edge | 1         | occupied, seen |
      | 13-16   | 0        |       | 1      |               |                     | 1         | occupied, seen |
    Then in sample 16 the bed posture is "sitting_up"

  Scenario: Feet at the side, doubtful, and no answer in time: out, on the safe side
    Given verification requests expire after 8 seconds
    And the verifier never answers
    And the resident came in alone and lay down in samples 1 to 10
    When the camera sees this sequence:
      | samples | on floor | where | in bed | subject moves | pose                | occupants | bed            |
      | 11      | 0        |       | 1      | yes           | lying               | 1         | occupied, seen |
      | 12      | 0        |       | 1      | yes           | sitting on the edge | 1         | occupied, seen |
      | 13-22   | 0        |       | 1      |               |                     | 1         | occupied, seen |
    Then in sample 19 the bed posture is "sitting_up"
    And in sample 20 the bed posture is "getting_out", seen "legs_out"
