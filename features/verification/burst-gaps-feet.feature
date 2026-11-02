# language: en

Feature: A feet question at its edges
  As care staff
  I want a doubtful ankle asked to the verifier to be taken as out only when
  the verifier says so or when its request ran out, whatever the way it was
  asked, so that a late or ambiguous answer never changes how the resident
  is seen getting out of bed (white book, volume III, ch. 10 and 12)

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

  Scenario: A feet question with a burst that gets no answer before its limit is out
    Given verification requests expire after 8 seconds
    And the verifier never answers
    And the feet are asked with 3 frames 2 seconds apart
    And the resident came in alone and lay down in samples 1 to 10
    When the camera sees this sequence:
      | samples | on floor | where | in bed | subject moves | pose                | occupants | bed            |
      | 11      | 0        |       | 1      | yes           | lying               | 1         | occupied, seen |
      | 12      | 0        |       | 1      | yes           | sitting on the edge | 1         | occupied, seen |
      | 13-30   | 0        |       | 1      |               | sitting on the edge | 1         | occupied, seen |
    Then in sample 16 a verification of the feet is asked
    And in sample 30 the bed posture is "getting_out"

  Scenario: Feet the verifier cannot tell stay doubtful: no call to get out
    Given the resident came in alone and lay down in samples 1 to 10
    And the verifier answers "unsure"
    When the camera sees this sequence:
      | samples | on floor | where | in bed | subject moves | pose                | occupants | bed            |
      | 11      | 0        |       | 1      | yes           | lying               | 1         | occupied, seen |
      | 12      | 0        |       | 1      | yes           | sitting on the edge | 1         | occupied, seen |
      | 13-20   | 0        |       | 1      |               | sitting on the edge | 1         | occupied, seen |
    Then in sample 12 a verification of the feet is asked
    And in sample 20 the bed posture is "sitting_up"

  Scenario: A late answer to a burst whose request already ran out changes nothing
    Given the feet are asked with 3 frames 2 seconds apart
    And verification requests expire after 4 seconds
    And the verifier answers after 6 seconds of source time
    And the resident came in alone and lay down in samples 1 to 10
    And the verifier answers "no"
    When the camera sees this sequence:
      | samples | on floor | where | in bed | subject moves | pose                | occupants | bed            |
      | 11      | 0        |       | 1      | yes           | lying               | 1         | occupied, seen |
      | 12      | 0        |       | 1      | yes           | sitting on the edge | 1         | occupied, seen |
      | 13-30   | 0        |       | 1      |               | sitting on the edge | 1         | occupied, seen |
    Then in sample 16 a verification of the feet is asked
    And in sample 26 the bed posture is "getting_out"
    And in sample 30 the bed posture is "getting_out"

  Scenario: An answer that arrives as the request runs out still counts
    Given verification requests expire after 4 seconds
    And the verifier answers after 4 seconds of source time
    And the resident came in alone and lay down in samples 1 to 10
    And the verifier answers "no"
    When the camera sees this sequence:
      | samples | on floor | where | in bed | subject moves | pose                | occupants | bed            |
      | 11      | 0        |       | 1      | yes           | lying               | 1         | occupied, seen |
      | 12      | 0        |       | 1      | yes           | sitting on the edge | 1         | occupied, seen |
      | 13-20   | 0        |       | 1      |               | sitting on the edge | 1         | occupied, seen |
    Then in sample 12 a verification of the feet is asked
    And in sample 20 the bed posture is "sitting_up"
