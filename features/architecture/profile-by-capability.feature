# language: en
@architecture @shell
Feature: A bad capability section is reported by its own name
  As whoever installs a camera
  I want a wrong section to say which capability it belongs to
  so as to fix the profile without reading the whole loader
  (ADR 2026-11-02, cascara-por-capacidad-y-nombres; R19 UC-A4)

  Scenario: A negative face confidence names the faces section
    Given a profile with "[faces]" and a minimum confidence of -0.1
    When the profile is checked
    Then the error names the section "[faces]"
    And the error names the field "min_confidence"

  # The profile is a test fixture with generic values: a site's profile is not the specification, and its values change with the site.
  Scenario: A valid profile with every capability loads, and its hash does not change
    Given the profile "crates/detector/tests/support/every-capability.toml" with every capability enabled
    When the profile is checked
    Then it loads
    And its hash is the one recorded before the reorganization
