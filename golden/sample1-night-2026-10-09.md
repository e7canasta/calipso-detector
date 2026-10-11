---
format: 1
cadence_s: 1
dwell_s: 2
senses: detect, reinforce, faces
---

# sample1 — first distilled night (2026-10-09)

Distilled from run `01M4H6QXX6SGBFKYRW23W881E5` of camera `sample1` (go2rtc, test
video; no pixels stored): 296 live samples with yolo26s, reinforcement yolo26l and
faces yolov12l-face, profile `config/sample1.toml`. The expectations are what the
engine believed live. Replayed through the detector with `config/scene-r6.toml`, it
matches in all 53 spans and publishes the same 20 domain facts (kind, cause and
resulting occupancy) as live.

| samples | on floor | where    | in bed | reinforcement in bed | faces        | occupants | bed            |
|---------|----------|----------|--------|----------------------|--------------|-----------|----------------|
| 1-53    | 0        |          | 0      |                      |              | 0         | empty          |
| 54-55   | 1        | near bed | 0      |                      | at headboard | 0         | empty          |
| 56-78   | 1        | near bed | 0      |                      | on subject   | 1         | empty          |
| 79      | 0        |          | 1      |                      | at headboard | 1         | empty          |
| 80      | 1        | near bed | 0      | 1                    |              | 1         | empty          |
| 81-82   | 0        |          | 1      |                      | at headboard | 1         | empty          |
| 83-150  | 0        |          | 1      |                      | on subject   | 1         | occupied, seen |
| 151-154 | 0        |          | 1      |                      |              | 1         | occupied, seen |
| 155-160 | 0        |          | 0      |                      |              | 1         | occupied, held |
| 161     | 0        |          | 1      |                      | at headboard | 1         | occupied, held |
| 162-167 | 0        |          | 0      |                      |              | 1         | occupied, held |
| 168-169 | 0        |          | 1      |                      | at headboard | 1         | occupied, held |
| 170     | 0        |          | 1      |                      | on subject   | 1         | occupied, seen |
| 171     | 0        |          | 0      | 1                    | on subject   | 1         | occupied, seen |
| 172     | 0        |          | 1      |                      | on subject   | 1         | occupied, seen |
| 173     | 0        |          | 0      | 1                    | on subject   | 1         | occupied, seen |
| 174-175 | 0        |          | 1      |                      | on subject   | 1         | occupied, seen |
| 176-177 | 0        |          | 0      |                      |              | 1         | occupied, seen |
| 178     | 0        |          | 1      |                      | on subject   | 1         | occupied, seen |
| 179     | 0        |          | 0      | 1                    | on subject   | 1         | occupied, seen |
| 180-186 | 0        |          | 0      |                      |              | 1         | occupied, held |
| 187-188 | 0        |          | 1      |                      | at headboard | 1         | occupied, held |
| 189-190 | 0        |          | 1      |                      | on subject   | 1         | occupied, seen |
| 191     | 0        |          | 0      | 1                    | on subject   | 1         | occupied, seen |
| 192     | 0        |          | 1      |                      | on subject   | 1         | occupied, seen |
| 193-198 | 0        |          | 0      |                      |              | 1         | occupied, held |
| 199-207 | 0        |          | 0      |                      | at headboard | 1         | occupied, held |
| 208-209 | 0        |          | 1      |                      | at headboard | 1         | occupied, held |
| 210-214 | 0        |          | 1      |                      | on subject   | 1         | occupied, seen |
| 215     | 0        |          | 0      |                      |              | 1         | occupied, seen |
| 216     | 0        |          | 1      |                      | on subject   | 1         | occupied, seen |
| 217     | 0        |          | 0      |                      |              | 1         | occupied, seen |
| 218     | 0        |          | 1      |                      | on subject   | 1         | occupied, seen |
| 219     | 0        |          | 0      | 1                    | on subject   | 1         | occupied, seen |
| 220-226 | 0        |          | 1      |                      | on subject   | 1         | occupied, seen |
| 227     | 0        |          | 0      | 1                    | on subject   | 1         | occupied, seen |
| 228-243 | 0        |          | 1      |                      | on subject   | 1         | occupied, seen |
| 244     | 0        |          | 0      | 1                    | on subject   | 1         | occupied, seen |
| 245     | 0        |          | 1      |                      | on subject   | 1         | occupied, seen |
| 246     | 0        |          | 0      | 1                    | on subject   | 1         | occupied, seen |
| 247     | 0        |          | 0      |                      | at headboard | 1         | occupied, seen |
| 248     | 0        |          | 0      |                      |              | 1         | occupied, seen |
| 249     | 0        |          | 0      |                      | at headboard | 1         | occupied, held |
| 250-252 | 0        |          | 0      |                      |              | 1         | occupied, held |
| 253     | 0        |          | 1      |                      | at headboard | 1         | occupied, held |
| 254-272 | 0        |          | 1      |                      |              | 1         | occupied, seen |
| 273     | 0        |          | 1      |                      | on subject   | 1         | occupied, seen |
| 274     | 0        |          | 1      |                      |              | 1         | occupied, seen |
| 275     | 0        |          | 1      |                      | on subject   | 1         | occupied, seen |
| 276-292 | 0        |          | 1      |                      |              | 1         | occupied, seen |
| 293     | 0        |          | 1      |                      | on subject   | 1         | occupied, seen |
| 294     | 0        |          | 1      |                      |              | 1         | occupied, seen |
| 295-296 | 0        |          | 1      |                      | on subject   | 1         | occupied, seen |
