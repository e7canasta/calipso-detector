# language: en
Feature: Layer 1 reads the posture from the frame, not from the depth
  As care staff
  I want the bed posture read from the three planes of the bed
  so that a resident lying along the bed is lying, a resident sat up is sitting up, and a head beside the headboard is not taken for getting out

  # F5.2 (spec: calipso-roadmap/en-curso/43-ocupacion-por-partes/specs/f5-2-la-capa-1-lee-el-marco.md).
  # With [occupancy.parts] and posture = true, the sightings come from the frame:
  # the torso (E3), the head against the headboard (E6) and the legs out of the
  # bed (E7). The scenes have no depth model: today's disparity is unknown, so
  # the posture does not change on it. The resident is seen in every sample
  # (no held state): the source is what changes.

  # The bed is a corner bed (F5.2, decision of the director): the mattress on the
  # diagonal, v at about 45 degrees in the image, the footprint 40 px lower, so up
  # stays vertical as on dig1. The body's box is centred on the mattress: 254,174
  # to 374,374.

  Background:
    Given a room with the bed delimited
    And the bed is a corner bed on the diagonal
    And the bed measures 0.9 by 2.0 metres
    And the geometry's shadow measures near the bed within 0.6 metres
    And the profile asks for the parts
    And the bed posture is read from the frame
    And stabilization requires 2 seconds of source time
    And samples arrive every 1 second of source time
    And there is a pose model
    And the bed posture is followed

  # Seen in bed, lying along the bed with the head on the pillow: lying. Today the
  # disparity is unknown and the posture stays in_bed. (red today)
  Scenario: A resident seen in bed, lying along it with the head on the pillow, is lying
    When in sample 1 the camera sees a person at 254,174 to 374,374
    And in sample 2 the camera sees a person at 254,174 to 374,374
    And in sample 3 the camera sees a person at 254,174 to 374,374
    And in sample 4 the camera sees a person at 254,174 to 374,374 and the pose sees them "corner: lying along the bed"
    And in sample 5 the camera sees a person at 254,174 to 374,374 and the pose sees them "corner: lying along the bed"
    And in sample 6 the camera sees a person at 254,174 to 374,374 and the pose sees them "corner: lying along the bed"
    Then in sample 6 the bed posture is "lying"

  # Lying, then sat up: sitting up. Today the posture stays lying. (red today)
  Scenario: A resident lying in bed who sits up against the headboard is sitting up
    When in sample 1 the camera sees a person at 254,174 to 374,374
    And in sample 2 the camera sees a person at 254,174 to 374,374
    And in sample 3 the camera sees a person at 254,174 to 374,374
    And in sample 4 the camera sees a person at 254,174 to 374,374 and the pose sees them "corner: lying along the bed"
    And in sample 5 the camera sees a person at 254,174 to 374,374 and the pose sees them "corner: lying along the bed"
    And in sample 6 the camera sees a person at 254,174 to 374,374 and the pose sees them "corner: lying along the bed"
    And in sample 7 the camera sees a person at 254,174 to 374,374 and the pose sees them "corner: sitting up against the headboard"
    And in sample 8 the camera sees a person at 254,174 to 374,374 and the pose sees them "corner: sitting up against the headboard"
    And in sample 9 the camera sees a person at 254,174 to 374,374 and the pose sees them "corner: sitting up against the headboard"
    Then in sample 9 the bed posture is "sitting_up"

  # Lying with the legs out past the near side: getting out, through the feet's
  # rule (E7). Red today: the posture never reaches lying in these scenes.
  Scenario: A resident lying in bed whose legs go out past the near side is getting out
    When in sample 1 the camera sees a person at 254,174 to 374,374
    And in sample 2 the camera sees a person at 254,174 to 374,374
    And in sample 3 the camera sees a person at 254,174 to 374,374
    And in sample 4 the camera sees a person at 254,174 to 374,374 and the pose sees them "corner: lying along the bed"
    And in sample 5 the camera sees a person at 254,174 to 374,374 and the pose sees them "corner: lying along the bed"
    And in sample 6 the camera sees a person at 254,174 to 374,374 and the pose sees them "corner: lying along the bed"
    And in sample 7 the camera sees a person at 254,174 to 374,374 and the pose sees them "corner: lying, feet out"
    And in sample 8 the camera sees a person at 254,174 to 374,374 and the pose sees them "corner: lying, feet out"
    And in sample 9 the camera sees a person at 254,174 to 374,374 and the pose sees them "corner: lying, feet out"
    Then in sample 9 the bed posture is "getting_out"

  # Guard (F5.1: BesideStop does not feed head_side): lying along the bed, the head
  # past the headboard's side on its plane but inside the mattress's sides. Not
  # getting out; still lying, and the head is not seen as head_side.
  Scenario: A resident lying with the head beside the headboard is not getting out
    When in sample 1 the camera sees a person at 254,174 to 374,374
    And in sample 2 the camera sees a person at 254,174 to 374,374
    And in sample 3 the camera sees a person at 254,174 to 374,374
    And in sample 4 the camera sees a person at 254,174 to 374,374 and the pose sees them "corner: lying along the bed"
    And in sample 5 the camera sees a person at 254,174 to 374,374 and the pose sees them "corner: lying along the bed"
    And in sample 6 the camera sees a person at 254,174 to 374,374 and the pose sees them "corner: lying along the bed"
    And in sample 7 the camera sees a person at 254,174 to 374,374 and the pose sees them "corner: lying, head beside the headboard"
    And in sample 8 the camera sees a person at 254,174 to 374,374 and the pose sees them "corner: lying, head beside the headboard"
    And in sample 9 the camera sees a person at 254,174 to 374,374 and the pose sees them "corner: lying, head beside the headboard"
    Then in sample 9 the bed posture is not "getting_out"
    And in sample 9 the bed posture was not seen as "head_side"

  # The shadow publishes the frame's posture next to the decision (ADR 2026-11-03):
  # the torso as the frame reads it, and the side the legs leave the bed by, when
  # they leave. Absent without posture = true.
  Scenario: The shadow publishes the torso along the bed, lying
    When in sample 1 the camera sees a person at 254,174 to 374,374 and the pose sees them "corner: lying along the bed"
    Then the body's parts' torso is "along_bed"

  Scenario: The shadow publishes the torso upright, sitting up against the headboard
    When in sample 1 the camera sees a person at 254,174 to 374,374 and the pose sees them "corner: sitting up against the headboard"
    Then the body's parts' torso is "upright"

  Scenario: The shadow publishes the side the legs leave the bed by
    When in sample 1 the camera sees a person at 254,174 to 374,374 and the pose sees them "corner: lying, feet out"
    Then the body's parts leave the bed by the "near" side
