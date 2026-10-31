# language: en
Feature: The room in two planes
  As whoever installs a corner camera that sees the bed on the diagonal
  I want the profile to choose a room measured on the mattress and on the floor
  so that whoever stands between the camera and the bed is not believed in bed

  # The scenes' bed is 100..500 x 100..400 and its footprint on the floor is
  # drawn 40 px lower, 100..500 x 140..440; the bed measures 0.9 by 2.0 metres:
  # 30 px past the footprint's foot is 0.1 of its length, 0.2 m (white book,
  # volume VII, ch. 14).

  Background:
    Given a room with the bed delimited
    And the bed's footprint on the floor is delimited as 100,140 to 500,440
    And the bed measures 0.9 by 2.0 metres

  Scenario: Without two planes the room is frontal: standing by the near side is in bed
    When in sample 1 the camera sees a person at 80,200 to 160,470
    Then the body is in the "bed"

  Scenario: In two planes, standing by the near side is on the floor, near the bed
    Given the room is measured in two planes: in bed with 0.45 of the box over the mattress and its support within 0.10 metres, near the bed within 0.60 metres
    When in sample 1 the camera sees a person at 80,200 to 160,470
    Then the body is on the "floor"
    And the body is near the bed

  Scenario: In two planes, lying in bed stays in bed
    Given the room is measured in two planes: in bed with 0.45 of the box over the mattress and its support within 0.10 metres, near the bed within 0.60 metres
    When in sample 1 the camera sees a person at 150,150 to 450,380
    Then the body is in the "bed"

  Scenario: In two planes, standing behind the headboard is on the floor, near the bed
    Given the room is measured in two planes: in bed with 0.45 of the box over the mattress and its support within 0.10 metres, near the bed within 0.60 metres
    When in sample 1 the camera sees a person at 200,20 to 300,130
    Then the body is on the "floor"
    And the body is near the bed
