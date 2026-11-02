# language: en
@architecture @shell
Feature: Each probe is answered by the port that owns it
  As the engine's maintainer
  I want every probe to reach exactly one answerer, chosen by its kind
  so as to add a sense without touching the verifier, and the other way round
  (ADR 2026-11-02, quien-responde-cada-probe; R19 UC-A1)

  Scenario: A sense probe is answered by the senses and never by the verifier
    Given a turn that asks for the main detection over the whole frame
    When the shell routes the probe
    Then the senses answer it
    And the verifier is not asked anything

  Scenario: A verification probe goes to the verifier, not to the senses
    Given a turn that asks to verify the held body with a question
    When the shell routes the probe
    Then the verifier takes the request
    And the senses are not asked anything

  Scenario: Keeping a crop is a verifier probe too
    Given a burst that keeps the crop of this sample
    When the shell routes the probe
    Then the verifier keeps the crop
    And the senses are not asked anything

  Scenario: The router measures the time once, whichever answers
    Given any probe
    When the shell routes it
    Then the answer carries one measured time
    And the time is measured by the router, not by the answerer

  Scenario: A sense the profile does not configure fails as a model failure
    Given a profile without a pose model
    And a turn that asks for a pose
    When the shell routes the probe
    Then the answer is a model failure
    And the turn still concludes with a scene
