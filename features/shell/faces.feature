# language: en
@shell
Feature: The face capability
  As an engine that decides to look for faces on the subject or at the headboard
  I want the shell to answer that probe with a face detector, over the requested region
  so the core's decision runs the same with a real model as with a scripted one

  Background:
    Given an image of 640×480
    And a scripted main model
    And a scripted face model

  Scenario: The face probe runs the face model over the requested crop
    When the core asks for faces at the headboard 100,100 to 500,170
    Then the face model received an image of 400×70
    And the main model was not called

  Scenario: Faces come back in whole-image coordinates
    Given the face model sees a face at 180,10 to 220,50 of the crop
    When the core asks for faces at the headboard 100,100 to 500,170
    Then the reading carries a face at 280,110 to 320,150

  Scenario: The probe on the subject's box crops that box
    When the core asks for faces on the subject 150,150 to 250,350
    Then the face model received an image of 100×200

  Scenario: Faces return everything the model sees, without policy
    Given the face model sees 3 faces, one of confidence 0.05
    When the core asks for faces at the headboard 100,100 to 500,170
    Then the reading carries 3 boxes

  Scenario: Without a face model, the probe fails like an unavailable sense
    Given there is no face model
    When the core asks for faces at the headboard 100,100 to 500,170
    Then the reading fails because of the model

  Scenario: The profile with faces turns on the core policy
    Given a profile with "[senses.faces]" of model "face/yolov12l-face.onnx" and minimum confidence 0.30
    Then the core policy accepts faces from 0.30
    And the published profile names the senses "detect, faces"

  Scenario: With reinforcement and faces, the profile names them in order
    Given a profile with "[senses.reinforce]" and "[senses.faces]"
    Then the published profile names the senses "detect, reinforce, faces"

  Scenario: The profile rejects a face backend stricter than the policy
    Given a profile with "[senses.faces]" with backend confidence 0.40 and minimum confidence 0.30
    Then the profile is rejected with a reason that names "senses.faces.backend_confidence"

  Scenario: End to end, a face at the headboard reaches the scene
    Given a detector with faces, headboard and scripted models
    And the main model sees nobody
    And the face model sees a face at the headboard
    When samples 1 to 3 arrive
    Then the face model was called 3 times
    And the scene of sample 3 carries 1 accepted face
    And every published message validates against its schema
