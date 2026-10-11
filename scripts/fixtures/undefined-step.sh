# Rule 15: a scenario with an undefined step must make the harness fail.
cat > features/occupancy/fixture-without-steps.feature <<'FEATURE'
# language: en
Feature: Fixture
  Scenario: A step nobody defined
    Given a step that does not exist in the harness
FEATURE
