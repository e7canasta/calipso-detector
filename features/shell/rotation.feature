# language: en
@shell
Feature: The frame turned before anything looks at it
  As whoever installs a camera that hangs turned (galia, hab. 306)
  I want the profile to turn the frame as it arrives
  so that the senses, the map and the scene all work on the upright frame

  # [stream] rotate: a quarter turn clockwise, in degrees. The frame turns once,
  # right after it is decoded and before any sense looks at it; the map is
  # measured on the turned frame, so map.resolution is the turned size and the
  # core's resolution gate (F-09) refuses a map measured on the other one.
  # ADR 2026-11-04, el cuadro se gira al llegar.

  Scenario: Without rotate the frame arrives as it was decoded
    Given a profile whose stream does not turn
    When a 4 by 2 frame marked at its top-left corner arrives
    Then the driver receives a 4 by 2 frame marked at its top-left corner

  Scenario: A quarter turn clockwise
    Given a profile whose stream has "rotate = 90"
    When a 4 by 2 frame marked at its top-left corner arrives
    Then the driver receives a 2 by 4 frame marked at its top-right corner

  Scenario: A half turn
    Given a profile whose stream has "rotate = 180"
    When a 4 by 2 frame marked at its top-left corner arrives
    Then the driver receives a 4 by 2 frame marked at its bottom-right corner

  Scenario: Three quarters clockwise
    Given a profile whose stream has "rotate = 270"
    When a 4 by 2 frame marked at its top-left corner arrives
    Then the driver receives a 2 by 4 frame marked at its bottom-left corner

  Scenario: A turn that is not a quarter is refused
    Given a profile whose stream has "rotate = 45"
    Then the profile is rejected with a reason that names "stream.rotate"

  Scenario: Without rotate the profile keeps its hash
    Given a profile whose stream does not turn
    Then the profile's hash is "sha256:239a5803f0c15901"

  Scenario: A turn is part of what the profile hashes
    Given a profile whose stream has "rotate = 90"
    Then the profile's hash is not "sha256:239a5803f0c15901"
