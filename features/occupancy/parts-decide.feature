# language: en
Feature: The parts decide the zone, with memory (behind a switch)
  As whoever installs a corner camera with the room's parts on
  I want the parts to say who is in bed, with the memory of what was proven
  so that a covered body is not dropped from bed, and a body that got up is not kept in it

  # F4.1 (spec: calipso-roadmap/en-curso/43-ocupacion-por-partes/specs/f4-1-la-pieza-decide.md).
  # The scenes' bed is 100..500 x 100..400, its footprint 100..500 x 140..440.
  # With decide = false, the box decides as it does today; with decide = true,
  # the cascade decides, and the memory keeps what was proven.

  Background:
    Given a room with the bed delimited
    And the bed's footprint on the floor is delimited as 100,140 to 500,440
    And the bed measures 0.9 by 2.0 metres
    And the geometry's shadow measures near the bed within 0.6 metres
    And the profile asks for the parts
    And there is a pose model

  Scenario: With decide off, the box decides: a body whose ankles are guessed is in bed
    When in sample 1 the camera sees a person at 150,150 to 450,380 and the pose sees them "lying, feet covered"
    Then the body is in the "bed"

  # Standing behind the bed: the box is over the mattress, and the ankles the
  # model sees are inside the mattress in the image but not proof (low score:
  # "un tobillo visto de verdad", ch. 05). Looked at, not proven: the floor.
  Scenario: With the parts deciding, a body standing behind the bed with guessed ankles is on the floor
    Given the parts decide the zone
    When in sample 1 the camera sees a person at 150,150 to 450,380 and the pose sees them "standing tall, feet covered"
    Then the body is on the "floor"
    And the body's parts are on the "floor", by the step "unproven"

  Scenario: With the parts deciding, a lying body with covered ankles is on the floor, not proven
    Given the parts decide the zone
    When in sample 1 the camera sees a person at 150,150 to 450,380 and the pose sees them "lying, feet covered"
    Then the body is on the "floor"
    And the body's parts are on the "floor", by the step "unproven"

  Scenario: With the parts deciding, a covered body that proved the bed stays in bed by memory
    Given the parts decide the zone
    When in sample 1 the camera sees a person at 150,150 to 450,380 and the pose sees them "lying"
    And in sample 2 the camera sees a person at 150,150 to 450,380 and the pose sees them "lying, feet covered"
    Then the body is in the "bed"

  # The figure's ankles sit 90 px under the box's centre: the box 150,180 to
  # 450,450 puts them at y 405, past the mattress's foot (400), on the floor;
  # the box's support stays within the footprint (box in bed by the box). The
  # hips are a mattress height and a half over the ankles: standing, whatever
  # the memory says.
  Scenario: With the parts deciding, a body that got up with its feet on the floor leaves the bed despite memory
    Given the parts decide the zone
    When in sample 1 the camera sees a person at 150,150 to 450,380 and the pose sees them "lying"
    And in sample 2 the camera sees a person at 150,180 to 450,450 and the pose sees them "standing tall"
    Then the body is on the "floor"
    And the body's parts are on the "floor", by the step "standing"

  # Sitting on the edge: the feet on the floor under the knees, the pelvis a
  # mattress height above them (E2).
  Scenario: With the parts deciding, a body sitting on the edge with its pelvis at the mattress height is in bed
    Given the parts decide the zone
    When in sample 1 the camera sees a person at 200,250 to 300,480 and the pose sees them "sitting on the edge, feet below"
    Then the body is in the "bed"
    And the body's parts are on the "bed", by the step "pelvis_at_edge"

  # One pose per turn: the bigger body is asked; the other is not looked at, and
  # without memory it is where its box puts it (F4.1 (a)).
  Scenario: With the parts deciding, a body whose pose was not looked at is where its box puts it
    Given the parts decide the zone
    When in sample 1 the camera sees a person at 150,150 to 450,380 and a second person at 60,150 to 180,380, the pose sees the first "lying" and does not look at the second
    Then the body at 60,150 to 180,380 is in the "bed"
    And the body at 60,150 to 180,380 has its parts by the step "pose_not_looked"

  # One memory, two bodies over it (F4.1 (b)): each remembered body is matched at
  # most once, greedy by the best IoU. The resident's box is remembered (150,150
  # to 450,420). Two detections with IoU of at least 0.5 are one body at
  # admission, so the two bodies overlap less than that between them (0.18), and
  # each overlaps the remembered box enough to match it: 150,150 to 330,420 (0.60)
  # and 270,150 to 450,480 (0.53). The first is looked at and lies in bed; the
  # second's pose is not looked at, and its box puts it on the floor (its support
  # is past the footprint's foot).
  Scenario: With the parts deciding, two bodies over one remembered bed body, only the best match inherits it
    Given the parts decide the zone
    When in sample 1 the camera sees a person at 150,150 to 450,420 and the pose sees them "lying"
    And in sample 2 the camera sees a person at 150,150 to 330,420 and a second person at 270,150 to 450,480, the pose sees the first "lying" and does not look at the second
    Then the body at 150,150 to 330,420 is in the "bed"
    And the body at 270,150 to 450,480 is in the "floor"
    And the body at 270,150 to 450,480 has its parts by the step "pose_not_looked"

  # The layer 0 gets the floor body's «near the bed» from the cascade (F2.2's rule
  # P2: the support within near_m of the footprint), not from the box. The
  # resident lies in bed for three samples; then the same kind of body stands at
  # the near side with its feet on the floor and its support 0.2 m from the
  # footprint: near the bed, so it is an exit from the bed alone, not an
  # accompanied room with someone held.
  Scenario: With the parts deciding, a resident who stands up at the near side leaves the bed alone
    Given the parts decide the zone
    When in sample 1 the camera sees a person at 150,150 to 450,380 and the pose sees them "lying"
    And in sample 2 the camera sees a person at 150,150 to 450,380 and the pose sees them "lying"
    And in sample 3 the camera sees a person at 150,150 to 450,380 and the pose sees them "lying"
    And in sample 4 the camera sees a person at 80,200 to 160,470 and the pose sees them "standing tall"
    And in sample 5 the camera sees a person at 80,200 to 160,470 and the pose sees them "standing tall"
    And in sample 6 the camera sees a person at 80,200 to 160,470 and the pose sees them "standing tall"
    And in sample 7 the camera sees a person at 80,200 to 160,470 and the pose sees them "standing tall"
    Then an exit from the bed was observed
    And nobody is held in bed
    And events.v5 published "room_changed" from "alone_in_bed_seen" to "alone_out"
