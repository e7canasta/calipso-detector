# language: en
@shell
Feature: The shell publishes the contract versions its profile chooses
  As the owner of a camera
  I want the profile to say which versions of the contract the shell publishes
  so a consumer moves to v5 when it is ready, and v4 stays byte for byte until then

  Scenario: By default the shell publishes v4 only
    Given a detector with reinforcement and scripted models
    And a scripted main model
    And the main model sees the resident in bed in samples 1 to 4 and nobody after
    When samples 1 to 4 arrive
    Then every published message is of contract v4
    And every published message validates against its schema

  Scenario: A profile that lists v5 publishes both versions, each with its own numbering
    Given a detector with reinforcement and scripted models and publish versions "v4" and "v5"
    And a scripted main model
    And the main model sees the resident in bed in samples 1 to 4 and nobody after
    When samples 1 to 4 arrive
    Then the messages of contract v4 and of contract v5 are both published
    And the event numbers of v5 start at 1 and are contiguous
    And every published message validates against its schema

  Scenario: The scene of v5 says the last event numbered before it
    Given a detector with reinforcement and scripted models and publish versions "v4" and "v5"
    And a scripted main model
    And the main model sees the resident in bed in samples 1 to 4 and nobody after
    When samples 1 to 4 arrive
    Then the scene of v5 says last_event_seq equal to the number of its last event

  Scenario: A profile that names an unknown version is rejected
    Given a profile with publish versions "v9"
    Then the profile is rejected with a reason that names "publish.versions"

  # Transitorio (C2): un perfil que lista solo v5 es válido. C4 quita la sección
  # [publish] y este escenario sale con ella.
  Scenario: A profile that lists v5 alone is accepted
    Given a profile with publish versions "v5"
    Then the profile is accepted

  Scenario: A reception change has no observed_at in v5, and v4 keeps the last sample's
    Given a detector with reinforcement and scripted models and publish versions "v4" and "v5"
    And a scripted main model
    And the main model sees the resident in bed in samples 1 to 4 and nobody after
    When samples 1 to 4 arrive and then the stream disconnects
    Then the reception change of v5 has no observed_at
    And the reception change event of v5 says at of sample 4
    And the reception change of v4 says observed_at of sample 4

  Scenario: A reception change before any sample names no sample in v5, and v4 names sequence 0
    Given a detector with reinforcement and scripted models and publish versions "v4" and "v5"
    And a scripted main model
    When the stream disconnects before any sample
    Then the reception change of v5 has no observed_at
    And the reception change event of v5 has no at
    And the reception change of v4 says observed_at of sample 0
