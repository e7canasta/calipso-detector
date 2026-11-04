# language: en
Feature: The reading of the parts
  As care staff
  I want to know what the places of the resident's parts mean: lying, sitting, on the edge, trying to leave
  so that "in bed" says how they are in it, from occupancy alone

  # The semantic sub-state of the parts' trama (white book, volume I, ch.
  # 10), decided by a list over six facts in this order, as
  # contract/parts-reading-table.txt says: out of bed; on the edge (the groin
  # in, the box out); lying (the shoulders and the face or the head at the
  # headboard, nothing out); trying to leave (something out while some torso
  # is at the headboard); sitting up (everything in, the whole boxes in, the
  # headboard seen free); sitting in bed (the headboard seen free); otherwise
  # undecided. An arm out is not "out", nor a face, a head or shoulders at the
  # headboard. The urgent readings (trying to leave, on the edge) hold half of
  # layer 0's dwell.

  Background:
    Given a room with the bed delimited
    And the bed's headboard delimited down to the shoulders
    And stabilization requires 2 seconds of source time
    And samples arrive every 1 second of source time

  Rule: The places say how they are in bed

    Scenario: Lying in sight reads lying
      Given there is a pose model
      And the resident came in alone and lay down in samples 1 to 10
      When the camera sees this sequence:
        | samples | on floor | where | in bed | subject moves | pose  | occupants | bed            |
        | 11      | 0        |       | 1      | yes           | lying | 1         | occupied, seen |
        | 12-13   | 0        |       | 1      |               |       | 1         | occupied, seen |
      Then in sample 13 the parts read "lying"

    Scenario: An arm out is not leaving: still lying
      Given there is a pose model
      And the resident came in alone and lay down in samples 1 to 10
      When the camera sees this sequence:
        | samples | on floor | where | in bed | subject moves | pose       | occupants | bed            |
        | 11      | 0        |       | 1      | yes           | an arm out | 1         | occupied, seen |
        | 12-13   | 0        |       | 1      |               |            | 1         | occupied, seen |
      Then in sample 13 the parts read "lying"

    Scenario: The legs out while the torso is at the headboard: trying to leave, and it counts sooner
      Given there is a pose model
      And the resident came in alone and lay down in samples 1 to 10
      When the camera sees this sequence:
        | samples | on floor | where | in bed | subject moves | pose                | occupants | bed            |
        | 11      | 0        |       | 1      | yes           | lying               | 1         | occupied, seen |
        | 12-13   | 0        |       | 1      |               |                     | 1         | occupied, seen |
        | 14      | 0        |       | 1      | yes           | sitting on the edge | 1         | occupied, seen |
        | 15-16   | 0        |       | 1      |               |                     | 1         | occupied, seen |
      Then in sample 14 the parts read "lying"
      And in sample 15 the parts read "trying_to_leave"

    # The head of someone sitting up is high: it projects out of the bed, and
    # nothing is left at the headboard.
    Scenario: Sitting up projects the head out of the bed: sitting in bed
      Given there is a pose model
      And the resident came in alone and lay down in samples 1 to 10
      When the camera sees this sequence:
        | samples | on floor | where | in bed | subject moves | pose       | occupants | bed            |
        | 11      | 0        |       | 1      | yes           | sitting up | 1         | occupied, seen |
        | 12-13   | 0        |       | 1      |               |            | 1         | occupied, seen |
      Then in sample 13 the parts read "sitting_in_bed"

    # Slid down, the head leaves the headboard with everything still in bed:
    # the places are those of sitting up, which occupancy cannot tell apart.
    Scenario: Everything in bed and the headboard seen free: sitting up
      Given there is a pose model
      And the resident came in alone and lay down in samples 1 to 10
      When the camera sees this sequence:
        | samples | on floor | where | in bed | subject moves | pose      | occupants | bed            |
        | 11      | 0        |       | 1      | yes           | slid down | 1         | occupied, seen |
        | 12-13   | 0        |       | 1      |               |           | 1         | occupied, seen |
      Then in sample 13 the parts read "sitting_up"

    # Frontal puts a body in bed by its box's centre, so the centre is always
    # in while seen. In two planes the share and the support decide: a box
    # 0.48 over the mattress, its centre past the far side, the hips split on
    # the edge. The bed is 100..500 x 100..400.
    Scenario: In two planes, the groin in the bed and the box's centre out: on the edge, and it counts sooner
      Given the bed's footprint on the floor is delimited as 100,140 to 500,440
      And the bed measures 0.9 by 2.0 metres
      And the room is measured in two planes: in bed with 0.45 of the box over the mattress and its support within 0.10 metres, near the bed within 0.60 metres
      And there is a pose model
      When in samples 1 to 4 the camera sees a person at 452,150 to 552,390 and the pose sees them "lying"
      Then in sample 4 the parts read "on_the_edge"

    Scenario: Before getting in, the resident at the door reads out of bed
      Given there is a pose model
      And the resident came in alone and lay down in samples 1 to 10
      Then in sample 5 the parts read "out_of_bed"

  Rule: What occupancy cannot tell, it does not say

    # Against the headboard the parts are where lying puts them: occupancy
    # reads lying. Telling them apart is the depth's work, in layer 1.
    Scenario: Sitting up against the headboard reads lying; the depth of layer 1 tells it apart
      Given there is a pose model
      And there is a depth model
      And the motion threshold is 0.1 of the subject's diagonal
      And the bed posture is followed
      And the resident came in alone and lay down in samples 1 to 10
      When the camera sees this sequence:
        | samples | on floor | where | in bed | subject moves | pose                             | occupants | bed            |
        | 11      | 0        |       | 1      | yes           | lying                            | 1         | occupied, seen |
        | 12      | 0        |       | 1      | yes           | sitting up against the headboard | 1         | occupied, seen |
        | 13-14   | 0        |       | 1      |               |                                  | 1         | occupied, seen |
      Then in sample 14 the parts read "lying"
      And in sample 12 the bed posture is "sitting_up", seen "headboard_upright"

    # A corner camera sees the board stand over the mattress's head edge: the
    # headboard is drawn from its top down over the pillow, and a head against
    # it falls out of the mattress. At the headboard is in bed: it is not
    # "something out" (ADR 2026-11-04, en la cabecera es en la cama).
    Scenario: The head against a headboard drawn above the mattress is not out of bed: lying
      Given the bed's headboard delimited from above the mattress down to the shoulders
      And there is a pose model
      And the resident came in alone and lay down in samples 1 to 10
      When the camera sees this sequence:
        | samples | on floor | where | in bed | subject moves | pose       | occupants | bed            |
        | 11      | 0        |       | 1      | yes           | sitting up | 1         | occupied, seen |
        | 12-13   | 0        |       | 1      |               |            | 1         | occupied, seen |
      Then in sample 13 the parts read "lying"

    Scenario: Without a pose the headboard is never seen: no reading in bed
      Given there is no pose model
      And the resident came in alone and lay down in samples 1 to 10
      Then in sample 10 the parts read "undecided"

    Scenario: Covered, the reading holds on what was retained
      Given there is a pose model
      And the resident came in alone and lay down in samples 1 to 10
      When the camera sees this sequence:
        | samples | on floor | where | in bed | subject moves | pose  | occupants | bed            |
        | 11      | 0        |       | 1      | yes           | lying | 1         | occupied, seen |
        | 12-13   | 0        |       | 1      |               |       | 1         | occupied, seen |
        | 14-20   | 0        |       | 0      |               |       | 1         | occupied, held |
      Then in sample 20 the parts read "lying"
