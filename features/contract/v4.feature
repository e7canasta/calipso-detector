# language: en
Feature: Contract v4: the transition and the mode
  As someone reading a night
  I want each layer-0 change as a row with its before and after, and a scene that states the mode
  to tell the night in cells without reimplementing the engine's rules

  Background:
    Given a room with the bed delimited
    And stabilization requires 2 seconds of source time
    And samples arrive every 1 second of source time

  Scenario: Each fact states the occupancy it leaves
    Given the resident came in alone and lay down in samples 1 to 10
    When the camera sees this sequence:
      | samples | on floor | where    | in bed | occupants | bed            |
      | 11      | 1        | door     | 1      | 1         | occupied, seen |
      | 12-13   | 1        | near bed | 0      | 1         | occupied, seen |
      | 14-15   | 1        | near bed | 0      | 2         | occupied, held |
    Then layer 0 published transition "110:10:211" with these facts:
      | fact        | leaves | mode              |
      | held_in_bed | 111    | alone_in_bed_held |
      | entered     | 211    | accompanied       |

  Scenario: The scene states the mode and links the last cell
    Given the resident came in alone and lay down in samples 1 to 10
    Then the scene of sample 10 says mode "alone_in_bed_seen", phase "running" and last cell "100:01:110"

  Scenario: room_changed goes by mode
    Given the resident came in alone and lay down in samples 1 to 10
    Then events.v4 published "room_changed" from "alone_out" to "alone_in_bed_seen"

  Scenario: A two-digit count is written as a tuple
    # The cell's format, not an engine behaviour: the scenes' world
    # models at most two people on the floor.
    Given a transition from "12,0,0" seeing "13,0" to "13,0,0"
    Then its cell is "(12,0,0):(13,0):(13,0,0)"
    Given a transition from "9,1,1" seeing "10,0" to "10,1,1"
    Then its cell is "911:(10,0):(10,1,1)"

  Scenario: Every transition agrees with its facts and its scene
    Given the staff put the resident to bed and left in samples 1 to 25
    Then on every turn, each transition agrees with its facts and its scene
