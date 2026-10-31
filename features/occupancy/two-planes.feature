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

  # The knee (white book, volume VII, ch. 15): a body the box leaves on the
  # floor by its support, with most of its box over the mattress, is doubtful;
  # its pose is asked before believing, and a bent knee puts it in bed. Sitting
  # on the foot of the bed, 200,250 to 300,480: 0.65 of the box over the
  # mattress, its support 40 px past the footprint's foot, 0.27 m.

  Scenario: In two planes with the knee, sitting on the foot of the bed with the knee bent is in bed
    Given the room is measured in two planes: in bed with 0.45 of the box over the mattress and its support within 0.10 metres, near the bed within 0.60 metres
    And there is a pose model
    And a doubtful body is in bed with the knee bent to 150 degrees or less
    When in sample 1 the camera sees a person at 200,250 to 300,480 and the pose sees them "sitting on the edge"
    Then the pose was asked for a doubtful body
    And the body is in the "bed"
    And the body's knee measured at most 150 degrees

  Scenario: In two planes with the knee, standing by the foot with the leg straight stays on the floor, near the bed
    Given the room is measured in two planes: in bed with 0.45 of the box over the mattress and its support within 0.10 metres, near the bed within 0.60 metres
    And there is a pose model
    And a doubtful body is in bed with the knee bent to 150 degrees or less
    When in sample 1 the camera sees a person at 200,250 to 300,480 and the pose sees them "standing"
    Then the pose was asked for a doubtful body
    And the body is on the "floor"
    And the body is near the bed
    And the body's knee measured more than 150 degrees

  Scenario: In two planes with the knee, a doubtful body without its ankles in sight stays on the floor
    Given the room is measured in two planes: in bed with 0.45 of the box over the mattress and its support within 0.10 metres, near the bed within 0.60 metres
    And there is a pose model
    And a doubtful body is in bed with the knee bent to 150 degrees or less
    When in sample 1 the camera sees a person at 200,250 to 300,480 and the pose sees them "lying, feet covered"
    Then the pose was asked for a doubtful body
    And the body is on the "floor"
    And the body's knee was not measured

  Scenario: In two planes with the knee, a doubtful body stays on the floor if the pose fails
    Given the room is measured in two planes: in bed with 0.45 of the box over the mattress and its support within 0.10 metres, near the bed within 0.60 metres
    And there is a pose model
    And a doubtful body is in bed with the knee bent to 150 degrees or less
    When in sample 1 the camera sees a person at 200,250 to 300,480 and the pose fails
    Then the pose for a doubtful body failed
    And the body is on the "floor"

  Scenario: In two planes with the knee, lying in bed is not doubtful
    Given the room is measured in two planes: in bed with 0.45 of the box over the mattress and its support within 0.10 metres, near the bed within 0.60 metres
    And there is a pose model
    And a doubtful body is in bed with the knee bent to 150 degrees or less
    When in sample 1 the camera sees a person at 150,150 to 450,380 and the pose sees them "sitting on the edge"
    Then the pose was not asked for any doubtful body
    And the body is in the "bed"

  Scenario: In two planes without the knee, no pose is asked before believing
    Given the room is measured in two planes: in bed with 0.45 of the box over the mattress and its support within 0.10 metres, near the bed within 0.60 metres
    And there is a pose model
    When in sample 1 the camera sees a person at 200,250 to 300,480 and the pose sees them "sitting on the edge"
    Then the pose was not asked for any doubtful body
    And the body is on the "floor"

  Scenario: In two planes with the knee, sitting on the foot of the bed is the subject, posed once
    Given the room is measured in two planes: in bed with 0.45 of the box over the mattress and its support within 0.10 metres, near the bed within 0.60 metres
    And there is a pose model
    And a doubtful body is in bed with the knee bent to 150 degrees or less
    When in sample 1 the camera sees a person at 200,250 to 300,480 and the pose sees them "sitting on the edge"
    And in sample 2 the camera sees a person at 200,250 to 300,480 and the pose sees them "sitting on the edge"
    And in sample 3 the camera sees a person at 200,250 to 300,480 and the pose sees them "sitting on the edge"
    And in sample 4 the camera sees a person at 200,250 to 300,480 and the pose sees them "sitting on the edge"
    Then the pose was asked for a doubtful body
    And there is a subject, in bed
    And in sample 4 the pose was not requested because it was already seen as a doubtful body
