# language: en
Feature: The layer-0 transition is published
  As someone reading a night
  I want to receive each layer-0 change as a row of the table
  to tell the night in cells and build episodes without reimplementing the rules

  Background:
    Given a room with the bed delimited
    And stabilization requires 2 seconds of source time
    And samples arrive every 1 second of source time

  Scenario: They put them to bed and leave: four rows in order, with their modes
    When the camera sees this sequence:
      | samples | on floor | where    | in bed | occupants | bed            |
      | 1-2     | 2        | door     | 0      | 0         | empty          |
      | 3-5     | 2        | door     | 0      | 2         | empty          |
      | 6-7     | 1        | near bed | 1      | 2         | empty          |
      | 8-10    | 1        | near bed | 1      | 2         | occupied, seen |
      | 11-12   | 1        | near bed | 0      | 2         | occupied, seen |
      | 13-15   | 1        | near bed | 0      | 2         | occupied, held |
      | 16-17   | 0        |          | 0      | 2         | occupied, held |
      | 18-20   | 0        |          | 0      | 1         | occupied, held |
    Then layer 0 published these transitions, in order:
      | cell       | before      | after             | facts            |
      | 000:20:200 | empty       | accompanied       | entered, entered |
      | 200:11:210 | accompanied | accompanied       | lay_down         |
      | 210:10:211 | accompanied | accompanied       | held_in_bed      |
      | 211:00:111 | accompanied | alone_in_bed_held | left_room        |

  Scenario: The dwell of the previous state
    When the camera sees this sequence:
      | samples | on floor | where | in bed | occupants | bed            |
      | 1-2     | 1        | door  | 0      | 0         | empty          |
      | 3-5     | 1        | door  | 0      | 1         | empty          |
      | 6-7     | 0        |       | 1      | 1         | empty          |
      | 8-20    | 0        |       | 1      | 1         | occupied, seen |
      | 21-22   | 0        |       | 0      | 1         | occupied, seen |
      | 23-25   | 0        |       | 0      | 1         | occupied, held |
    Then transition "110:00:111" says what we believed lasted 15 seconds of source time

  Scenario: The span and its context: the carer comes in and covers them (S8)
    Given the resident came in alone and lay down in samples 1 to 10
    When the camera sees this sequence:
      | samples | on floor | where    | in bed | occupants | bed            |
      | 11      | 1        | door     | 1      | 1         | occupied, seen |
      | 12-13   | 1        | near bed | 0      | 1         | occupied, seen |
      | 14-15   | 1        | near bed | 0      | 2         | occupied, held |
    Then layer 0 published transition "110:10:211"
    And its span started at sample 12 and saw 1 person far from the bed

  Scenario: A flicker publishes nothing
    Given the resident came in alone and lay down in samples 1 to 10
    When the camera sees this sequence:
      | samples | on floor | where | in bed | occupants | bed            |
      | 11      | 0        |       | 0      | 1         | occupied, seen |
      | 12-15   | 0        |       | 1      | 1         | occupied, seen |
    Then layer 0 published no transitions since sample 11

  Scenario: After a reconnection, the dwell is unknown
    Given the resident came in alone and lay down in samples 1 to 10
    And the camera reconnects before sample 11
    When the camera sees this sequence:
      | samples | on floor | where | in bed | occupants | bed            |
      | 11-12   | 0        |       | 0      | 1         | occupied, seen |
      | 13-15   | 0        |       | 0      | 1         | occupied, held |
    Then transition "110:00:111" does not say how long what we believed lasted
