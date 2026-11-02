# expect-step: tests
# expect-text: Given a shell step that does not exist in the harness
# Rule 15 in the shell: a @shell scenario with an undefined step must
# make the detector harness fail.
cat > features/shell/fixture-without-steps.feature <<'FEATURE'
# language: en
@shell
Feature: Shell fixture
  Scenario: A shell step nobody defined
    Given a shell step that does not exist in the harness
FEATURE
