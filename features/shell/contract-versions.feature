# language: en
@shell
Feature: The shell publishes contract v5, and only v5
  As the owner of a camera
  I want the shell to publish the contract a consumer reads
  so the wire says what the engine concludes, and nothing else

  Scenario: The shell publishes only v5, with no publish section
    Given a detector with reinforcement and scripted models
    And a scripted main model
    And the main model sees the resident in bed in samples 1 to 4 and nobody after
    When samples 1 to 4 arrive
    Then every published message is of contract v5
    And every published message validates against its schema

  Scenario: The v5 events are numbered from 1 and are contiguous
    Given a detector with reinforcement and scripted models
    And a scripted main model
    And the main model sees the resident in bed in samples 1 to 4 and nobody after
    When samples 1 to 4 arrive
    Then the event numbers of v5 start at 1 and are contiguous

  Scenario: The scene of v5 says the last event numbered before it
    Given a detector with reinforcement and scripted models
    And a scripted main model
    And the main model sees the resident in bed in samples 1 to 4 and nobody after
    When samples 1 to 4 arrive
    Then the scene of v5 says last_event_seq equal to the number of its last event

  Scenario: A reception change has no observed_at in v5
    Given a detector with reinforcement and scripted models
    And a scripted main model
    And the main model sees the resident in bed in samples 1 to 4 and nobody after
    When samples 1 to 4 arrive and then the stream disconnects
    Then the reception change of v5 has no observed_at
    And the reception change event of v5 says at of sample 4

  Scenario: A reception change before any sample names no sample in v5
    Given a detector with reinforcement and scripted models
    And a scripted main model
    When the stream disconnects before any sample
    Then the reception change of v5 has no observed_at
    And the reception change event of v5 has no at

  Scenario: A profile with a publish section is rejected as an unknown field
    Given a profile with publish versions "v5"
    Then the profile is rejected with a reason that names "unknown field `publish`"
