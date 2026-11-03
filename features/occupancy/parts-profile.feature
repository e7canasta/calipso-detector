# language: en
@shell @occupancy
Feature: The room's parts in the profile
  As whoever installs a corner camera that sees the bed on the diagonal
  I want the profile to ask for the room's parts with its own section
  so that a profile that cannot read the parts does not start

  # The section needs the room in two planes, the pose, the faces and the
  # headboard as four points, in the bed's order (white book, volume IX, ch. 09).

  Scenario: The parts section loads on a two-plane profile with the head's stops
    Given the parts profile with the section ""
    Then the profile is accepted

  Scenario: Without two planes the parts section is refused
    Given the parts profile with "geometry = \"two_planes\"" replaced by "geometry = \"frontal\""
    Then the profile is rejected with a reason that names "[occupancy.parts]"

  Scenario: Without the pose the parts section is refused
    Given the parts profile with "[senses.pose]\nmodel = \"yolo26s-pose.onnx\"\n" replaced by ""
    Then the profile is rejected with a reason that names "[senses.pose]"

  Scenario: Without the faces the parts section is refused
    Given the parts profile with "[senses.faces]\nmodel = \"face/yolov12l-face.onnx\"\n" replaced by ""
    Then the profile is rejected with a reason that names "[senses.faces]"

  Scenario: A headboard that is a box, not four points, is refused by the parts section
    Given the parts profile with "headboard = [[951, 365], [1281, 325], [1270, 425], [940, 465]]" replaced by "headboard = [733, 126, 1146, 340]"
    Then the profile is rejected with a reason that names "four points"

  Scenario: A headboard in the wrong order is refused
    Given the parts profile with "headboard = [[951, 365], [1281, 325], [1270, 425], [940, 465]]" replaced by "headboard = [[940, 465], [1270, 425], [1281, 325], [951, 365]]"
    Then the profile is rejected with a reason that names "headboard"

  Scenario: A far strip below zero does not load
    Given the parts profile with the section "far_strip_m = -0.1"
    Then the profile is rejected with a reason that names "far_strip_m"

  Scenario: An inverted pelvis band does not load
    Given the parts profile with the section "pelvis_band = [1.4, 0.6]"
    Then the profile is rejected with a reason that names "pelvis_band"
