# language: en
@shell
Feature: The depth capability
  As an engine that decides to measure the subject's depth and learn the empty bed
  I want the shell to answer those probes with a depth model, at the requested points
  so the core's decision runs the same with a real model as with a scripted one

  Background:
    Given an image of 640×480
    And a scripted main model
    And a scripted depth model

  Scenario: The depth probe runs the model over the whole image
    Given the depth model sees a 640×480 map worth 3.0 everywhere
    When the core asks for the subject's depth at points 10,10; 600,400
    Then the depth model received an image of 640×480
    And the main model was not called

  Scenario: The reading carries one value per point, in the requested order
    Given the depth model sees a 640×480 map worth 3.0 everywhere
    And the map is 2.5 at 10,10 and 4.0 at 600,400
    When the core asks for the subject's depth at points 600,400; 10,10; 300,200
    Then the reading carries the depths 4.0; 2.5; 3.0

  Scenario: A point outside the image has no value and the rest stay aligned
    Given the depth model sees a 640×480 map worth 3.0 everywhere
    When the core asks for the subject's depth at points 700,10; 10,10; -1,5; 640,480
    Then the reading carries the depths none; 3.0; none; 3.0

  Scenario: A non-finite or non-positive value is not a reading
    Given the depth model sees a 640×480 map worth 3.0 everywhere
    And the map is NaN at 10,10 and infinity at 20,20
    And the map is 0.0 at 30,30 and -1.0 at 40,40
    When the core asks for the subject's depth at points 10,10; 20,20; 30,30; 40,40; 50,50
    Then the reading carries the depths none; none; none; none; 3.0

  Scenario: A map smaller than the image is sampled proportionally
    Given the depth model sees a 64×48 map worth 3.0 everywhere
    And the map is 1.5 at 1,1 and 6.0 at 63,47
    When the core asks for the subject's depth at points 15,15; 639,479
    Then the reading carries the depths 1.5; 6.0

  Scenario: The same capability measures the empty bed
    Given the depth model sees a 640×480 map worth 3.0 everywhere
    And the map is 2.5 at 10,10 and 4.0 at 600,400
    When the core asks for the bed's depth at points 10,10; 600,400
    Then the reading carries the depths 2.5; 4.0
    And the depth model received an image of 640×480

  Scenario: Over a region, the model sees only that crop and the points stay in whole-image coordinates
    Given the depth model sees a 400×200 map worth 3.0 everywhere
    And the map is 2.5 at 10,10
    When the core asks for the subject's depth at points 110,60; 10,10 over 100,50 to 500,250
    Then the depth model received an image of 400×200
    And the reading carries the depths 2.5; none

  Scenario: Without a depth model, the probe fails like an unavailable sense
    Given there is no depth model
    When the core asks for the bed's depth at points 10,10; 600,400
    Then the reading fails because of the model
    And the main model was not called

  Scenario: If the depth model fails, the reading fails
    Given the depth model fails
    When the core asks for the subject's depth at points 10,10; 600,400
    Then the reading fails because of the model

  Scenario: The profile with pose and depth measures the subject
    Given a profile with pose and "[senses.depth]" of model "yolo26s-depth.onnx"
    Then the core policy measures the subject's depth at keypoints with score 0.3 or more
    And the core does not calibrate the bed
    And the published profile names the senses "detect, pose, depth"

  Scenario: The profile with calibration learns the bed with depth
    Given a profile with "[senses.depth]" of model "yolo26s-depth.onnx" and "[calibration]" retrying every 30 seconds
    Then the core policy calibrates the bed and retries after 30 seconds of source time
    And the core does not request the pose
    And the published profile names the senses "detect, calibrate"

  Scenario: The profile measures depth over the bed
    Given a profile with pose, depth and a depth crop of 0.25
    Then the core measures depth over the bed grown by 0.25

  Scenario: Without a depth crop, depth is measured over the whole frame
    Given a profile with pose and "[senses.depth]" of model "yolo26s-depth.onnx"
    Then the core measures depth over the whole frame

  Scenario: The profile rejects a negative depth crop
    Given a profile with pose, depth and a depth crop of -0.1
    Then the profile is rejected with a reason that names "senses.depth.crop_pad"

  Scenario: The profile rejects calibrating without depth
    Given a profile with "[calibration]" without "[senses.depth]"
    Then the profile is rejected with a reason that names "calibration"

  Scenario: The profile rejects a depth nobody asks for
    Given a profile with "[senses.depth]" without pose or calibration
    Then the profile is rejected with a reason that names "senses.depth"

  Scenario: The profile rejects an invalid retry delay
    Given a profile with "[senses.depth]" of model "yolo26s-depth.onnx" and "[calibration]" retrying every -1 seconds
    Then the profile is rejected with a reason that names "calibration.retry_secs"

  Scenario: End to end, the empty room calibrates the bed once
    Given a detector with calibration and scripted models
    And the main model sees nobody
    And the depth model sees a 160×96 map worth 3.0 everywhere
    When samples 1 to 5 arrive
    Then the depth model was called 1 time, in sample 1, with the whole image
    And in sample 1 it was published that the bed was calibrated at 3.0
    And every published message validates against its schema

  Scenario: End to end, the new pose is measured at its keypoints
    Given a detector with pose, depth and scripted models
    And a scripted pose model
    And the main model sees the resident still in bed
    And the pose model sees a person with a keypoint at 10,20 of the crop
    And the depth model sees a 160×96 map worth 3.0 everywhere
    And the map is 2.0 at 40,50
    When samples 1 to 8 arrive
    Then the depth model was called 1 time, in sample 3, with the whole image
    And the scene of sample 3 carries the subject at depth 2.0
    And every published message validates against its schema
