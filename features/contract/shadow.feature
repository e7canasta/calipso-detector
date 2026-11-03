# language: en
Feature: The geometry's shadow is published next to what is decided
  As whoever designs the engine for a corner camera
  I want each scene to carry where the bed and "near the bed" fall for a geometry that does not decide yet
  to choose with data which technique decides, before changing a single decision

  # The scenes' bed is 100..500 x 100..400: u runs along x (400 px), v along y
  # (300 px). With a bed of 0.9 by 2.0 metres, 0.6 m is 0.667 of its width
  # (267 px) and 0.3 of its length (90 px). Today, near the bed is the bed's
  # bounding box grown by 0.1: 60..540 x 70..430 (white book, volume VII, ch. 11).

  Background:
    Given a room with the bed delimited
    And stabilization requires 2 seconds of source time
    And samples arrive every 1 second of source time

  Scenario: Without the shadow in the profile, the scene carries none
    Given the resident came in alone and lay down in samples 1 to 10
    Then the scene's map carries no shadow

  Scenario: The map's shadow: the footprint, the size, near in metres and near as today
    Given the bed's footprint on the floor is delimited as 100,140 to 500,440
    And the bed measures 0.9 by 2.0 metres
    And the geometry's shadow measures near the bed within 0.6 metres
    And the resident came in alone and lay down in samples 1 to 10
    Then the scene's map shadow carries the bed's footprint as 100,140 to 500,440
    And the scene's map shadow carries a bed of 0.9 by 2.0 metres
    And the scene's map shadow carries near the bed as -167,50 to 767,530
    And the scene's map shadow carries near the bed today as 60,70 to 540,430

  Scenario: Without the footprint, near is measured on the mattress
    Given the bed measures 0.9 by 2.0 metres
    And the geometry's shadow measures near the bed within 0.6 metres
    And the resident came in alone and lay down in samples 1 to 10
    Then the scene's map shadow carries no footprint
    And the scene's map shadow carries near the bed as -167,10 to 767,490

  Scenario: Without the bed's size there are no metres
    Given the geometry's shadow measures near the bed within 0.6 metres
    And the resident came in alone and lay down in samples 1 to 10
    Then the scene's map shadow carries no near in metres
    And the scene's map shadow carries near the bed today as 60,70 to 540,430

  Scenario: The shadow does not change what is believed
    Given the bed's footprint on the floor is delimited as 100,140 to 500,440
    And the bed measures 0.9 by 2.0 metres
    And the geometry's shadow measures near the bed within 0.6 metres
    And the resident came in alone and lay down in samples 1 to 10
    When the camera sees this sequence:
      | samples | on floor | where    | in bed | occupants | bed            |
      | 11-15   | 0        |          | 0      | 1         | occupied, held |
      | 16-20   | 0        |          | 1      | 1         | occupied, seen |
      | 21-25   | 1        | near bed | 0      | 1         | empty          |
    Then the same night without the shadow gives the same scenes, but for the shadow, and the same events

  # Stage 2, each body. The footprint 100..500 x 140..440 is the bed drawn 40 px
  # lower. The support is the centre of the box's lower edge, measured on the
  # footprint; 30 px past its foot is 0.1 of its length: 0.2 m.

  Scenario: Standing by the near side: in bed today, on the floor for the shadow
    Given the bed's footprint on the floor is delimited as 100,140 to 500,440
    And the bed measures 0.9 by 2.0 metres
    And the geometry's shadow measures near the bed within 0.6 metres
    When in sample 1 the camera sees a person at 80,200 to 160,470
    Then the body is in the "bed" today and on the "floor" for the shadow
    And the body's support falls 0.2 metres from the bed, on the "floor"
    And the body's support is near the bed for the shadow

  Scenario: Lying in bed: in bed today and for the shadow
    Given the bed's footprint on the floor is delimited as 100,140 to 500,440
    And the bed measures 0.9 by 2.0 metres
    And the geometry's shadow measures near the bed within 0.6 metres
    When in sample 1 the camera sees a person at 150,150 to 450,380
    Then the body is in the "bed" today and in the "bed" for the shadow
    And the body's support falls 0 metres from the bed, on the "floor"
    And the body's share over the mattress is 1

  Scenario: Without the footprint the support is measured on the mattress
    Given the bed measures 0.9 by 2.0 metres
    And the geometry's shadow measures near the bed within 0.6 metres
    When in sample 1 the camera sees a person at 80,200 to 160,470
    Then the body's support falls 0.47 metres from the bed, on the "mattress"

  Scenario: Without the shadow the bodies carry none
    When in sample 1 the camera sees a person at 80,200 to 160,470
    Then the body carries no shadow

  # Stage 3, the parts in the shadow (ADR 2026-11-03, las partes en la sombra).
  # With [occupancy.parts] the body carries its zone by parts and the step that
  # decided it, and nothing else changes: the zone that counts is still `zone`.
  # A standing body with no pose looked at: its box touches the bed, no face decides,
  # so the pose is asked; it is looked at and proves nothing: "unproven", on the floor.

  Scenario: With the parts in the profile, a standing body publishes its zone by parts
    Given the bed's footprint on the floor is delimited as 100,140 to 500,440
    And the bed measures 0.9 by 2.0 metres
    And the geometry's shadow measures near the bed within 0.6 metres
    And the profile asks for the parts
    When in sample 1 the camera sees a person at 80,200 to 160,470
    Then the body is in the "floor" today and on the "floor" for the shadow
    And the body's parts are on the "floor", by the step "unproven"
    And the body's parts turn by "bed_axis"

  Scenario: Without the parts in the profile, the body carries none
    Given the bed's footprint on the floor is delimited as 100,140 to 500,440
    And the bed measures 0.9 by 2.0 metres
    And the geometry's shadow measures near the bed within 0.6 metres
    When in sample 1 the camera sees a person at 80,200 to 160,470
    Then the body's shadow carries no parts

  # F2.3b, the turn of the pose (criterion 2): the direction the pose is asked
  # with, by the rule that chose it. The pose is asked for the body the cascade
  # cannot place without it, bed or floor; one per turn.

  Scenario: A body mostly over the mattress asks its pose turned up the bed
    Given the bed's footprint on the floor is delimited as 100,140 to 500,440
    And the bed measures 0.9 by 2.0 metres
    And the geometry's shadow measures near the bed within 0.6 metres
    And the profile asks for the parts
    When in sample 1 the camera sees a person at 150,150 to 450,380
    Then the body's pose is asked pointing up the bed

  Scenario: A face alone in its box turns the pose to the face
    Given the bed's footprint on the floor is delimited as 100,140 to 500,440
    And the bed measures 0.9 by 2.0 metres
    And the geometry's shadow measures near the bed within 0.6 metres
    And the profile asks for the parts
    And there is a face detector with minimum confidence 0.3
    When in sample 1 the camera sees a person at 150,150 to 450,380 and a face at 200,200
    Then the body's pose is asked pointing from 300,265 to 200,200

  Scenario: A body with little of its box over the mattress asks its pose with no direction
    Given the bed's footprint on the floor is delimited as 100,140 to 500,440
    And the bed measures 0.9 by 2.0 metres
    And the geometry's shadow measures near the bed within 0.6 metres
    And the profile asks for the parts
    When in sample 1 the camera sees a person at 90,330 to 130,460
    Then the body's pose is asked with no direction
