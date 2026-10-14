# language: en
Feature: The occupancy of the parts
  As care staff
  I want to know where each part of the resident is against the bed and the headboard, and what is kept unseen
  so as to tell lying from sitting on the edge, leaning out or a leg out, even under the blanket

  # The sister of layer 0, at its compass: once per sample, with its dwell.
  # Its row is l0[t] + places[t-1] : seen[t] : places[t], decided position by
  # position as contract/parts-table.txt says. The trama:
  #
  #   seen:  bed IN.OUT : headboard IN.OUT : BFK            (- : not looked)
  #   state: bed IN.OUT.HELD : headboard IN.OUT.HELD : BFK.HELD
  #   bed cfhsaip: box centre, face, head, shoulders, arms, groin, legs
  #   headboard fhs: face, head, shoulders
  #   BFK: the person's box all in the bed, the face's in the bed, the face's in the headboard
  #
  # Each part is a group of points: some inside, some outside; both, split.
  # A part not seen keeps where it was last seen, held, until it is seen
  # again or layer 0 loses the subject (white book, volume I, ch. 09).

  Background:
    Given a room with the bed delimited
    And the bed's headboard delimited
    And stabilization requires 2 seconds of source time
    And samples arrive every 1 second of source time
    And there is a pose model
    And the motion threshold is 0.1 of the subject's diagonal

  Rule: What is seen is where the parts are

    Scenario: Lying in sight: everything in bed, the head at the headboard
      Given the resident came in alone and lay down in samples 1 to 10
      When the camera sees this sequence:
        | samples | on floor | where | in bed | subject moves | pose  | occupants | bed            |
        | 11      | 0        |       | 1      | yes           | lying | 1         | occupied, seen |
        | 12-13   | 0        |       | 1      |               |       | 1         | occupied, seen |
      Then in sample 11 the parts saw "1-11111.0-00000:-10.-01:1--"
      And in sample 13 the parts are "1011111.0000000.0011111:010.001.011:100.000"

    Scenario: An arm out: the arms split, one in the bed and the other out
      Given the resident came in alone and lay down in samples 1 to 10
      When the camera sees this sequence:
        | samples | on floor | where | in bed | subject moves | pose       | occupants | bed            |
        | 11      | 0        |       | 1      | yes           | an arm out | 1         | occupied, seen |
      Then in sample 11 the parts saw "1-11111.0-00100:-10.-01:1--"

    Scenario: Sitting on the edge: the legs out of the bed, the rest in it
      Given the resident came in alone and lay down in samples 1 to 10
      When the camera sees this sequence:
        | samples | on floor | where | in bed | subject moves | pose                | occupants | bed            |
        | 11      | 0        |       | 1      | yes           | sitting on the edge | 1         | occupied, seen |
      Then in sample 11 the parts saw "1-11110.0-00001:-10.-01:1--"

    Scenario: Leaning out over the side: the head and the shoulders out of the bed and the headboard
      Given the resident came in alone and lay down in samples 1 to 10
      When the camera sees this sequence:
        | samples | on floor | where | in bed | subject moves | pose        | occupants | bed            |
        | 11      | 0        |       | 1      | yes           | leaning out | 1         | occupied, seen |
      Then in sample 11 the parts saw "1-00111.0-11000:-00.-11:1--"

  Rule: What is not seen is kept where it was last seen, held

    Scenario: Covered: every part stays where it was seen, held
      Given the resident came in alone and lay down in samples 1 to 10
      When the camera sees this sequence:
        | samples | on floor | where | in bed | subject moves | pose  | occupants | bed            |
        | 11      | 0        |       | 1      | yes           | lying | 1         | occupied, seen |
        | 12-13   | 0        |       | 1      |               |       | 1         | occupied, seen |
        | 14-20   | 0        |       | 0      |               |       | 1         | occupied, held |
      Then in sample 20 the parts saw "0------.0------:---.---:---"
      And in sample 20 the parts are "1011111.0000000.1011111:010.001.011:100.100"

    # Lying down, the pose saw the head in bed, below the headboard: it is
    # kept there, held, while the face is seen at the headboard.
    Scenario: Held with the face at the headboard: the face is seen, the rest is kept
      Given there is a face detector with minimum confidence 0.30
      And the resident came in alone and lay down in samples 1 to 10
      When the camera sees this sequence:
        | samples | on floor | where | in bed | faces        | occupants | bed            |
        | 11-20   | 0        |       | 0      | at headboard | 1         | occupied, held |
      Then in sample 20 the parts saw "01-----.00-----:1--.0--:-11"
      And in sample 20 the parts are "1110000.0000000.1010000:100.010.010:111.100"

    Scenario: When layer 0 loses the subject, what was kept is forgotten
      Given the resident came in alone and lay down in samples 1 to 10
      When the camera sees this sequence:
        | samples | on floor | where | in bed | subject moves | pose  | occupants | bed            |
        | 11      | 0        |       | 1      | yes           | lying | 1         | occupied, seen |
        | 12-13   | 0        |       | 1      |               |       | 1         | occupied, seen |
        | 14-17   | 1        | door  | 1      |               |       | 2         | occupied, seen |
      Then in sample 13 the parts are "1011111.0000000.0011111:010.001.011:100.000"
      And in sample 17 there are no parts
