---
format: 1
cadence_s: 1
dwell_s: 2
---

# S7 — Someone enters while the resident is covered

At startup someone is next to the bed and the resident is in it; they cover up and
another person comes in. The retained occupant could not leave without reappearing:
there are three.

| samples | on floor | where    | in bed | occupants | bed            |
|---------|----------|----------|--------|-----------|----------------|
| 1-5     | 1        | near bed | 1      | 2         | occupied, seen |
| 6-10    | 1        | near bed | 0      | 2         | occupied, held |
| 11-15   | 2        | door     | 0      | 3         | occupied, held |
