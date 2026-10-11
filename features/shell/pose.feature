# language: en
@shell
Feature: The pose capability
  As an engine that decides to take the resident's posture when they move
  I want the shell to answer that probe with a pose model, over the requested box
  so the core's decision runs the same with a real model as with a scripted one

  Background:
    Given an image of 640×480
    And a scripted main model
    And a scripted pose model

  Scenario: The pose probe runs the pose model over the requested box
    When the core asks for the pose in the box 60,70 to 540,430
    Then the pose model received an image of 480×360
    And the main model was not called

  Scenario: Pose keypoints come back in whole-image coordinates
    Given the pose model sees a person with a keypoint at 10,20 of the crop
    When the core asks for the pose in the box 60,70 to 540,430
    Then the reading carries a pose with a keypoint at 70,90

  Scenario: The pose returns all its keypoints, without policy
    Given the pose model sees a person with 17 keypoints, one with score 0.05
    When the core asks for the pose in the box 60,70 to 540,430
    Then the reading carries a pose of 17 keypoints
    And the reading carries a keypoint with score 0.05

  Scenario: With two people in the crop, the reading carries the most confident
    Given the pose model sees a person of confidence 0.40 and another of confidence 0.80
    When the core asks for the pose in the box 60,70 to 540,430
    Then the reading carries the pose of the person of confidence 0.80

  Scenario: If the model sees nobody, the reading says there is no pose
    Given the pose model sees nobody
    When the core asks for the pose in the box 60,70 to 540,430
    Then the reading says there is no pose

  Scenario: Without a pose model, the probe fails like an unavailable sense
    Given there is no pose model
    When the core asks for the pose in the box 60,70 to 540,430
    Then the reading fails because of the model
    And the main model was not called

  Scenario: If the pose model fails, the reading fails
    Given the pose model fails
    When the core asks for the pose in the box 60,70 to 540,430
    Then the reading fails because of the model

  Scenario: A box without pixels does not call the model
    When the core asks for the pose in the box 700,500 to 800,600
    Then the reading fails because of the model
    And the pose model was not called

  Scenario: The profile with pose turns on the core policy
    Given a profile with "[senses.pose]" of model "yolo26s-pose.onnx", motion threshold 0.1 and minimum score 0.3
    Then the core policy requests the pose when the subject moves 0.1 of their diagonal
    And the core policy does not measure the subject's depth
    And the published profile names the senses "detect, pose"

  Scenario: A profile without pose or depth does not change its hash
    Given a profile without "[senses.pose]"
    Then the profile hash is the baseline one
    And the core neither requests the pose nor calibrates the bed

  Scenario: The profile rejects an invalid motion threshold
    Given a profile with "[senses.pose]" of model "yolo26s-pose.onnx", motion threshold -0.1 and minimum score 0.3
    Then the profile is rejected with a reason that names "senses.pose.move_threshold"

  Scenario: End to end, the resident lying still is looked at once
    Given a detector with pose and scripted models
    And the main model sees the resident still in bed
    And the pose model sees a person with a keypoint at 10,20 of the crop
    When samples 1 to 8 arrive
    Then the pose model was called 1 time, in sample 3, with the resident's box
    And the scene of sample 3 carries the current pose with a keypoint at 40,50
    And the scene of sample 8 carries the pose stable since sample 3
    And every published message validates against its schema
