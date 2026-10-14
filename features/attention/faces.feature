# language: en
Feature: The face policy
  As care staff
  I want the engine to look for faces where they make sense: on whoever it sees, or at the headboard if it sees nobody
  to have a second evidence of presence without looking for faces in the whole image

  Background:
    Given a room with the bed delimited
    And the bed's headboard delimited
    And stabilization requires 2 seconds of source time
    And samples arrive every 1 second of source time
    And there is a face detector with minimum confidence 0.30

  Scenario: With someone in sight, faces are looked for in their box
    When the camera sees this sequence:
      | samples | on floor | where | in bed | occupants | bed            |
      | 1-3     | 0        |       | 1      | 1         | occupied, seen |
    Then in sample 3 faces were looked for on the body seen in bed

  Scenario: With nobody in sight, faces are looked for at the headboard
    When the camera sees this sequence:
      | samples | on floor | where | in bed | occupants | bed   |
      | 1-3     | 0        |       | 0      | 0         | empty |
    Then in sample 3 faces were looked for at the headboard

  Scenario: With the staff on the floor and the resident in bed, faces are looked for on the resident
    Given the resident came in alone and lay down in samples 1 to 10
    When the camera sees this sequence:
      | samples | on floor | where | in bed | occupants | bed            |
      | 11-13   | 1        | door  | 1      | 2         | occupied, seen |
    Then in sample 13 faces were looked for on the body seen in bed

  Scenario: If the main detector fails, faces are looked for at the headboard
    Given the detector fails in samples 1 to 3
    Then in sample 3 faces were looked for at the headboard

  Scenario: Without a headboard and with nobody in sight, faces are not looked for, and it says why
    Given the bed's headboard is not delimited
    When the camera sees this sequence:
      | samples | on floor | where | in bed | occupants | bed   |
      | 1-3     | 0        |       | 0      | 0         | empty |
    Then in sample 3 faces were not looked for because there is nowhere to look

  # The headboard is where faces are looked for, not where the head must be: a
  # head that leaves it, covered and unseen, would never be found by its face.
  # The search grows by the margin; where the head is is still decided against
  # the exact headboard (project 33).

  Scenario: With nobody in sight, faces are looked for at the headboard grown by its margin
    Given faces are looked for at the headboard grown by 0.5
    When the camera sees this sequence:
      | samples | on floor | where | in bed | occupants | bed   |
      | 1-3     | 0        |       | 0      | 0         | empty |
    Then in sample 3 faces were looked for at the headboard grown by 0.5

  Scenario: A face beside the headboard, inside the margin, is found
    Given faces are looked for at the headboard grown by 0.5
    When the camera sees this sequence:
      | samples | on floor | where | in bed | faces            | occupants | bed   |
      | 1-3     | 0        |       | 0      | beside headboard | 0         | empty |
    Then the last sample accepted 1 face

  Scenario: Without a margin, a face beside the headboard is not found
    When the camera sees this sequence:
      | samples | on floor | where | in bed | faces            | occupants | bed   |
      | 1-3     | 0        |       | 0      | beside headboard | 0         | empty |
    Then the last sample accepted 0 faces

  Scenario: A face on the seen resident is marked as on the subject
    Given the resident came in alone and lay down in samples 1 to 10
    When the camera sees this sequence:
      | samples | on floor | where | in bed | faces      | occupants | bed            |
      | 11      | 0        |       | 1      | on subject | 1         | occupied, seen |
    Then the last sample accepted 1 face
    And the face of the last sample is on the subject

  Scenario: A weak face is discarded and it says why
    When the camera sees this sequence:
      | samples | on floor | where | in bed | faces | occupants | bed   |
      | 1-3     | 0        |       | 0      | weak  | 0         | empty |
    Then the last sample accepted 0 faces
    And the last sample discarded a face for low confidence

  Scenario: A face at the headboard, without a body, adds nobody
    When the camera sees this sequence:
      | samples | on floor | where | in bed | faces        | occupants | bed   |
      | 1-10    | 0        |       | 0      | at headboard | 0         | empty |
    Then the last sample accepted 1 face
    And the room is "Empty"

  Scenario: With reinforcement and faces, the order is main, reinforcement, faces
    Given there is a reinforcement detector
    And the resident came in alone and lay down in samples 1 to 10
    When the camera sees this sequence:
      | samples | on floor | where | in bed | reinforcement in bed | faces      | occupants | bed            |
      | 11      | 0        |       | 0      | 1                    | on subject | 1         | occupied, seen |
    Then in sample 11 the looks were "detect, reinforce, faces"
    And the face of the last sample is on the subject

  Scenario: A night with faces replays without a camera and gives the same
    Given the resident came in alone and lay down in samples 1 to 10
    When the camera sees this sequence:
      | samples | on floor | where | in bed | faces        | occupants | bed            |
      | 11-15   | 0        |       | 1      | on subject   | 1         | occupied, seen |
      | 16-25   | 0        |       | 0      | at headboard | 1         | occupied, held |
    Then the same night, replayed with the same readings, gives the same scenes and events
