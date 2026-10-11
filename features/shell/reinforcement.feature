# language: en
@shell
Feature: The reinforcement capability
  As an engine that decides to reinforce the bed when it loses someone
  I want the shell to answer that probe with a heavier detector, over the requested region
  so the core's decision runs the same with a real model as with a scripted one

  Background:
    Given an image of 640×480
    And a scripted main model
    And a scripted reinforcement model

  Scenario: The reinforcement probe runs the reinforcement model over the requested crop
    When the core asks to reinforce the region 60,70 to 540,430
    Then the reinforcement model received an image of 480×360
    And the main model was not called

  Scenario: Reinforcement boxes come back in whole-image coordinates
    Given the reinforcement model sees a person at 10,20 to 110,220 of the crop
    When the core asks to reinforce the region 60,70 to 540,430
    Then the reading carries a person at 70,90 to 170,290

  Scenario: Reinforcement returns everything it sees, without policy
    Given the reinforcement model sees a person of confidence 0.10 and a chair
    When the core asks to reinforce the region 60,70 to 540,430
    Then the reading carries 2 boxes

  Scenario: Without a reinforcement model, the probe fails like an unavailable sense
    Given there is no reinforcement model
    When the core asks to reinforce the region 60,70 to 540,430
    Then the reading fails because of the model
    And the main model was not called

  Scenario: If the reinforcement model fails, the reading fails
    Given the reinforcement model fails
    When the core asks to reinforce the region 60,70 to 540,430
    Then the reading fails because of the model

  Scenario: A region without pixels does not call the model
    When the core asks to reinforce the region 700,500 to 800,600
    Then the reading fails because of the model
    And the reinforcement model was not called

  Scenario: The profile with reinforcement turns on the core policy
    Given a profile with "[senses.reinforce]" of model "yolo26l.onnx" and padding 0.15
    Then the core policy reinforces the bed grown by 0.15
    And the published profile names the senses "detect, reinforce"

  Scenario: A profile without reinforcement does not change its hash
    Given a profile without "[senses.reinforce]"
    Then the profile hash is the baseline one

  Scenario: The profile rejects a reinforcement that hides what the policy would accept
    Given a profile with "[senses.reinforce]" with backend confidence 0.60 and policy minimum confidence 0.50
    Then the profile is rejected with a reason that names "senses.reinforce.backend_confidence"

  Scenario: End to end, the resident covers up and the detector reinforces once
    Given a detector with reinforcement and scripted models
    And the main model sees the resident in bed in samples 1 to 10 and nobody after
    And the reinforcement model sees the resident in bed
    When samples 1 to 14 arrive
    Then the reinforcement model was called 1 time, in sample 11
    And the scene of sample 11 says the bed is "seen"
    And every published scene validates against the v3 schema
