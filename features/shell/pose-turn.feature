# language: en
@shell
Feature: The pose over a turned box (F2.4)
  As an engine that knows where the bed's long side points
  I want the shell to turn the crop of a body's box so that a direction of the image points up
  so a pose model meets a lying body upright, and the keypoints still come back in whole-image coordinates

  Background:
    Given an image of 640×480
    And a scripted main model
    And a scripted pose model

  Scenario: A turned pose returns its keypoints in whole-image coordinates
    Given the pose model sees a person with a keypoint at 1,1 of the crop
    When the core asks for the pose in the box 100,100 to 200,300 turned so that 1,0 points up
    Then the reading carries a pose with a keypoint at 199,101

  Scenario: Without a turn the pose is the one of today
    Given the pose model sees a person with a keypoint at 10,20 of the crop
    When the core asks for the pose in the box 60,70 to 540,430
    Then the reading carries a pose with a keypoint at 70,90
