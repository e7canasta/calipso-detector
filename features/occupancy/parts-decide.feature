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

  # F4.1b (spec: f4-1b-salir-hay-que-probarlo.md). Standing (E2) with the box
  # still over the mattress is not proof of leaving: the memory of bed holds.
  # The figure's ankles sit 90 px under the box's centre: the box 150,180 to
  # 450,450 puts them at y 405, past the mattress's foot (400), so E2 reads
  # "standing" (the hips a mattress height and a half over the ankles), while
  # the box is still over the mattress. This is also a sitting body with its
  # legs forward (dig1, seconds 22-27): E2 reads it standing.
  Scenario: With the parts deciding, a body that E2 reads standing, with its box still over the mattress, stays in bed by memory
    Given the parts decide the zone
    When in sample 1 the camera sees a person at 150,150 to 450,380 and the pose sees them "lying"
    And in sample 2 the camera sees a person at 150,180 to 450,450 and the pose sees them "standing tall"
    Then the body is in the "bed"
    And the body's parts are on the "floor", by the step "standing"

  # Several turns of standing with the box over the mattress: each turn keeps the
  # memory of bed (dig1, seconds 31-48, the bed trapped by the layer 0).
  Scenario: With the parts deciding, a body that stays standing over the mattress for several turns stays in bed
    Given the parts decide the zone
    When in sample 1 the camera sees a person at 150,150 to 450,380 and the pose sees them "lying"
    And in sample 2 the camera sees a person at 150,180 to 450,450 and the pose sees them "standing tall"
    And in sample 3 the camera sees a person at 150,180 to 450,450 and the pose sees them "standing tall"
    And in sample 4 the camera sees a person at 150,180 to 450,450 and the pose sees them "standing tall"
    Then the body is in the "bed"
    And the body's parts are on the "floor", by the step "standing"

  # The box leaves the mattress (below its foot, no touch) and no memory matches
  # it (no overlap with the box of sample 1): the step 1c decides alone, the body
  # is on the floor. The release with memory is the one of the bed posture (F4.1b).
  Scenario: With the parts deciding, a body that got up and whose box leaves the mattress is on the floor
    Given the parts decide the zone
    When in sample 1 the camera sees a person at 150,150 to 450,380 and the pose sees them "lying"
    And in sample 2 the camera sees a person at 150,420 to 450,470 and the pose sees them "standing tall"
    Then the body is on the "floor"
    And the body's parts are on the "floor", by the step "no_touch"

  # Sitting on the edge: the feet on the floor under the knees, the pelvis a
  # mattress height above them (E2).
  Scenario: With the parts deciding, a body sitting on the edge with its pelvis at the mattress height is in bed
    Given the parts decide the zone
    When in sample 1 the camera sees a person at 200,250 to 300,480 and the pose sees them "sitting on the edge, feet below"
    Then the body is in the "bed"
    And the body's parts are on the "bed", by the step "pelvis_at_edge"

  # The step 1 with memory (F4.1b): the resident lies in bed, then gets up and
  # leaves by the near side (the left edge, x 100 in the test world), a little at
  # a time. Each box overlaps the last by an IoU of at least 0.5 and moves at
  # most a third of its width, so the memory is inherited turn after turn. While
  # the box is over the mattress E2 reads "standing" and the memory holds; at the
  # last turn the box has left the mattress (its points are off it, share 0.31 <
  # 0.45): the step 1c, no_touch, releases it and the body is on the floor.
  Scenario: With the parts deciding, a resident who gets up and leaves by the near side, a little at a time, is released by the step 1 with memory
    Given the parts decide the zone
    When in sample 1 the camera sees a person at 150,150 to 450,380 and the pose sees them "lying"
    And in sample 2 the camera sees a person at 150,180 to 450,450 and the pose sees them "standing tall"
    And in sample 3 the camera sees a person at 100,180 to 400,450 and the pose sees them "standing tall"
    And in sample 4 the camera sees a person at 50,180 to 350,450 and the pose sees them "standing tall"
    And in sample 5 the camera sees a person at 0,180 to 300,450 and the pose sees them "standing tall"
    And in sample 6 the camera sees a person at 0,180 to 160,450 and the pose sees them "standing tall"
    Then the body is on the "floor"
    And the body's parts are on the "floor", by the step "no_touch"

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
