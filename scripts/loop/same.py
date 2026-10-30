#!/usr/bin/env python3
"""Compares two recordings of the same looped clip, loop by loop, scene by
scene: the characterization test of a refactor (white book, volume VII,
ch. 12).

    same.py BASELINE.jsonl CANDIDATE.jsonl

A loop is one session of the source (the clip ends, the stream reconnects,
a new session starts at source second 0). Each full loop of the candidate is
compared with a full loop of the baseline sampled at the same phase (the
fraction of the second at which the samples fall: another phase means other
frames). What depends on the run is left out: run, sequence numbers, wall
clock, timings, the milliseconds of each look, event ids, and each sample
reference keeps only its source second.

Two known differences are not the engine's and are counted apart:
- the cold start: the first session of each recording, while the models load,
  drops samples (`overwritten` in the log) and looks at other frames; it is
  left out;
- the inherited "since": a state held from a previous loop keeps that loop's
  source second. A scene whose only differences are "since" fields is not a
  difference.

Exit 0 if every compared loop is otherwise identical, scenes and events; 1
otherwise.
"""
import json, sys
from collections import defaultdict

RUN = {"source", "seq", "observed_at", "published_at_ms", "timing", "last_event_seq", "event_id"}


def strip(x):
    if isinstance(x, dict):
        if "session" in x and "sequence" in x:
            return {"source_secs": round(x.get("source_secs", 0), 2)}
        return {k: strip(v) for k, v in x.items() if k != "spent_ms"}
    if isinstance(x, list):
        return [strip(v) for v in x]
    return x


def loops(path):
    scenes, events = defaultdict(dict), defaultdict(list)
    for line in open(path):
        m = json.loads(line)
        e = m["body"]
        o = e.get("observed_at") or {}
        session, t = o.get("session"), o.get("source_secs")
        if session is None or t is None:
            continue
        body = strip({k: v for k, v in e.items() if k not in RUN})
        if m["subject"].endswith(".scene.v4"):
            scenes[session][round(t, 1)] = body
        else:
            events[session].append((round(t, 1), json.dumps(body, sort_keys=True)))
    longest = max((max(v) for v in scenes.values()), default=0)
    cold = min(scenes, default=None)
    full = {s: v for s, v in scenes.items() if s != cold and min(v) < 1.5 and max(v) > longest - 5}
    return full, events


def diff(a, b, path=""):
    if isinstance(a, dict) and isinstance(b, dict):
        for k in sorted(set(a) | set(b)):
            yield from diff(a.get(k), b.get(k), f"{path}.{k}")
    elif isinstance(a, list) and isinstance(b, list) and len(a) == len(b):
        for i, (p, q) in enumerate(zip(a, b)):
            yield from diff(p, q, f"{path}[{i}]")
    elif a != b:
        yield path, a, b


def inherited(changes):
    """A state held from a previous loop keeps that loop's "since": not a difference."""
    return all(".since" in p or "_since" in p or ".last_seen" in p for p, _, _ in changes)


base, base_events = loops(sys.argv[1])
cand, cand_events = loops(sys.argv[2])
phase = lambda v: round(min(v) % 1, 1)
print("baseline loops: ", {s: f"phase {phase(v)}, {len(v)} scenes" for s, v in base.items()})
print("candidate loops:", {s: f"phase {phase(v)}, {len(v)} scenes" for s, v in cand.items()})
ok, compared = True, 0
for s, v in cand.items():
    ref = next((r for r in base if phase(base[r]) == phase(v)), None)
    if ref is None:
        print(f"candidate loop {s}: no baseline loop at phase {phase(v)}; record more loops")
        continue
    compared += 1
    common = sorted(set(v) & set(base[ref]))
    real, start = [], []
    for t in common:
        changes = list(diff(base[ref][t], v[t]))
        if changes:
            (start if inherited(changes) else real).append((t, changes))
    ev_real = [(a, b) for (ta, a), (tb, b) in zip(base_events[ref], cand_events[s]) if a != b and not inherited(list(diff(json.loads(a), json.loads(b))))]
    same_count = len(base_events[ref]) == len(cand_events[s])
    print(f"candidate loop {s} vs baseline loop {ref}: {len(common)} scenes, "
          f"{len(common) - len(real) - len(start)} identical, {len(start)} differ only in the inherited 'since', "
          f"{len(real)} DIFFER; events {len(cand_events[s])} vs {len(base_events[ref])}, {len(ev_real)} DIFFER")
    if real or ev_real or not same_count:
        ok = False
    for t, changes in real[:5]:
        print(f"  {t}s:")
        for p, a, b in changes[:8]:
            print(f"    {p}: {str(a)[:80]} | {str(b)[:80]}")
if compared == 0:
    print("nothing compared")
    ok = False
print("SAME" if ok else "NOT THE SAME")
sys.exit(0 if ok else 1)
