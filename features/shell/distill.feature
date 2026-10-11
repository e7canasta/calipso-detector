# language: en
@shell
Feature: Distilling rejects readings it cannot reproduce
  Scenario Outline: A shared box won by one sense does not erase the other's reading
    Given a night with two bodies in bed where "<winner>" wins the shared box in sample 4
    When the night is distilled
    Then the distillation rejects sample 4 with a reason that names "<loser>"

    Examples:
      | winner        | loser     |
      | reinforcement | detect    |
      | main          | reinforce |

  Scenario Outline: A sense failure is not distilled as an empty reading
    Given a night where "<sense>" fails in sample 4
    When the night is distilled
    Then the distillation rejects sample 4 with a reason that names "<name>"

    Examples:
      | sense         | name      |
      | reinforcement | reinforce |
      | faces         | faces     |

  Scenario: Reinforcement that looked without finding bodies stays enabled when replaying the night
    Given a night where reinforcement looked without finding bodies
    When the night is distilled
    Then the distillation keeps the reinforcement sense
    And the distilled night repeats the same scenes and events per sample

  Scenario: Faces that looked without finding any stay enabled when replaying the night
    Given a night where faces looked without finding any
    When the night is distilled
    Then the distillation keeps the face sense
    And the distilled night repeats the same scenes and events per sample
