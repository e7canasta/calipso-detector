# language: en
@architecture
Feature: Renaming the reception does not change what is published
  As a consumer of scene.v5
  I want the reception field to keep its name and values
  so as to keep reading the same contract after the internal rename
  (ADR 2026-11-02, cascara-por-capacidad-y-nombres; R19 UC-A3)

  Scenario: A reception change is published under the same sensor field
    Given a stream that reconnects
    When the shell reports the reception change
    Then the published scene carries the field "sensor" with the value "reconnecting"
    And the v5 scene schema is the frozen snapshot

  Scenario: A stalled stream is still reported as stalled
    Given a stream with no samples for longer than its stall timeout
    When the shell reports the stall
    Then the published scene carries the field "sensor" with the value "stalled"
