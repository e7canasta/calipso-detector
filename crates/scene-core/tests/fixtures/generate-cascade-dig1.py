"""Parity fixture for F2.2: classify_s1 with E5S, no memory, on dig1's body-seconds.

Reads the workshop's prototype (partes.py) and the pose cache; writes JSON to argv[1].
Nothing under workshop/ is written.
"""
import json
import sys

import numpy as np

sys.path.insert(0, "/home/care/mana-origin/calipso/workshop/43-ocupacion-por-partes/herramientas")
import partes as P  # noqa: E402

BASE = "/home/care/mana-origin/calipso/workshop/43-ocupacion-por-partes"
OUT = sys.argv[1]

# The sprint 1 proposal (reglas.md, F1.1d): strip 0.18 m, score 0.75, E2 0.6-1.4, share 0.45, E5 tolerance 0.12.
cfg = {"e2_lo": 0.6, "e2_hi": 1.4, "e4_band": 0.2, "e5_tol": 0.12, "kp_min": 0.5,
       "strip_m": 0.18, "score_test": 0.75, "share": 0.45, "e3_up": P.E3_UP, "e3_v": P.E3_V}
RULES = {"E2", "E5", "E5S", "E6", "A", "B", "D"}

prof = P.load_profile(BASE + "/perfiles/dig1.toml")
scene = P.Scene(prof)
persons_by_s = P.read_pose(BASE + "/cache/cuadro-entero/pose-dig1.jsonl")


def num(v):
    return None if v is None else float(v)


rows = []
for s in sorted(persons_by_s):
    for i, person in enumerate(persons_by_s[s]):
        body = P.body_of(person, scene, cfg["kp_min"])
        zone, step, proof, box_floor, d = P.classify_s1(body, scene, cfg, RULES)
        face = body["parts"]["H"]
        rows.append({
            "s": s,
            "i": i,
            "box": [round(float(v), 1) for v in (body["box"]["x1"], body["box"]["y1"],
                                                  body["box"]["x2"], body["box"]["y2"])],
            "kp": [[round(float(k[0]), 1), round(float(k[1]), 1), round(float(k[2]), 3)]
                   for k in body["kp"].tolist()],
            "face": None if face is None else [round(float(face[0]), 2), round(float(face[1]), 2)],
            "zone": zone,
            "step": step,
            "share_px": float(P.share_over_bed(body["box"], scene)),
            "share_d": num(d.get("share")),
            "cut": bool(d["cut"]),
            "past_near": bool(d["past_near"]),
            "touch": bool(d["touch"]),
            "pie_visto": bool(d["pie_visto"]),
            "foot_level": float(d["foot_level"]),
            "uF": num(d.get("uF")),
            "score_F": num(d.get("score_F")),
            "h_G": num(d.get("h_G")),
        })

with open(OUT, "w") as f:
    json.dump({"source": "workshop/43-ocupacion-por-partes/herramientas/partes.py classify_s1, "
                         "rules " + ",".join(sorted(RULES)) + ", no memory, cfg " + json.dumps(cfg),
               "bodies": rows}, f, separators=(",", ":"))
print(len(rows), "body-seconds ->", OUT)
