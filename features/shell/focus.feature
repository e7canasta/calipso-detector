# language: en
@shell
Feature: The focus capability and its profile
  As an engine that decides to look at the bed up close with the main detector
  I want the shell to answer the main probe over the requested region, and the profile to switch the focus on with its guarantee
  so the core's decision runs the same with a real model as with a scripted one, and no profile breaks the sentinel's guarantee

  Rule: The main probe over a region runs the main model over the crop

    Background:
      Given an image of 640×480
      And a scripted main model

    Scenario: The main probe over the focus runs the main model over the crop
      When the core asks the main detector to look at the region 0,25 to 600,475
      Then the main model received an image of 600×450

    Scenario: The main detector's boxes over the focus come back in whole-image coordinates
      Given the main model sees a person at 10,20 to 110,220 of the crop
      When the core asks the main detector to look at the region 60,70 to 540,430
      Then the reading carries a person at 70,90 to 170,290

  Rule: The profile switches the focus on, and keeps the sentinel no longer than the walk

    Scenario: Without "[senses.detect.focus]" the main detector always looks at the whole frame
      Given a profile without "[senses.detect.focus]"
      Then the core policy does not focus
      And the profile hash is the baseline one

    Scenario: The focus section switches the focus on
      Given a profile whose main detector focuses on the bed grown by 0.25, looking at the whole frame every 3 seconds, and staff walk 3 seconds from the door to the bed
      Then the core policy focuses on the bed grown by 0.25 and looks at the whole frame every 3 seconds

    Scenario: A sentinel longer than the walk is rejected: whoever walks in would reach the bed unseen
      Given a profile whose main detector focuses on the bed grown by 0.25, looking at the whole frame every 10 seconds, and staff walk 3 seconds from the door to the bed
      Then the profile is rejected with a reason that names "senses.detect.focus.wide_every_secs"

    Scenario: A negative pad is rejected
      Given a profile whose main detector focuses on the bed grown by -0.1, looking at the whole frame every 3 seconds, and staff walk 3 seconds from the door to the bed
      Then the profile is rejected with a reason that names "senses.detect.focus.pad"

    Scenario: The site map's focus must contain the bed
      Given a focusing profile whose map delimits the focus as 200,50 to 700,550
      Then the profile is rejected with a reason that names "map.focus"

    Scenario: The site map's focus reaches the core
      Given a focusing profile whose map delimits the focus as 50,50 to 700,550
      Then the core's site map focuses on 50,50 to 700,550
