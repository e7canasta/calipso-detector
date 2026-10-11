# language: en
@shell
Feature: The main backend respects the admission threshold
  Scenario: The profile rejects a main detector that hides bodies the policy would admit
    Given a profile with "[senses.detect]" with backend confidence 0.90 and policy minimum confidence 0.50
    Then the profile is rejected with a reason that names "senses.detect.backend_confidence"

  Scenario: The profile accepts a main backend with the same threshold as the policy
    Given a profile with "[senses.detect]" with backend confidence 0.50 and policy minimum confidence 0.50
    Then the profile accepts the main detector's threshold
