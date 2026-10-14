"""Streaming analyzer for Bevy tracing-chrome JSON traces.

Pairs B/E events per thread and aggregates total/self time per span name.
Usage: python3 analyze_trace.py <trace.json> [--min-ms N] [--top N] [--grep SUBSTR]
"""

import json
import sys
from collections import defaultdict


def main() -> None:
    path = sys.argv[1]
    min_ms = 0.0
    top = 40
    grep = None
    i = 2
    while i < len(sys.argv):
        if sys.argv[i] == "--min-ms":
            min_ms = float(sys.argv[i + 1])
            i += 2
        elif sys.argv[i] == "--top":
            top = int(sys.argv[i + 1])
            i += 2
        elif sys.argv[i] == "--grep":
            grep = sys.argv[i + 1]
            i += 2
        else:
            i += 1

    tot = defaultdict(float)
    selftot = defaultdict(float)
    cnt = defaultdict(int)
    maxd = defaultdict(float)
    stacks = defaultdict(list)  # tid -> [[name, ts, child_accum], ...]

    with open(path) as f:
        for line in f:
            line = line.strip()
            if not line or line in ("[", "]"):
                continue
            line = line.removesuffix(",")
            if '"ph":"B"' not in line and '"ph":"E"' not in line:
                continue
            try:
                ev = json.loads(line)
            except json.JSONDecodeError as err:
                print(f"warning: skipping malformed trace line: {err}", file=sys.stderr)
                continue
            ph = ev.get("ph")
            tid = ev.get("tid", 0)
            if ph == "B":
                stacks[tid].append([ev.get("name", "?"), ev["ts"], 0.0])
            elif ph == "E":
                st = stacks[tid]
                if not st:
                    continue
                name, ts, child = st.pop()
                dur = ev["ts"] - ts
                tot[name] += dur
                cnt[name] += 1
                selftot[name] += dur - child
                maxd[name] = max(maxd[name], dur)
                if st:
                    st[-1][2] += dur

    order = sorted(tot, key=lambda k: tot[k], reverse=True)
    print(f"{'total_ms':>12} {'self_ms':>12} {'max_ms':>10} {'count':>8}  name")
    shown = 0
    for name in order:
        if tot[name] / 1000.0 < min_ms:
            continue
        if grep and grep not in name:
            continue
        print(
            f"{tot[name] / 1000:12.1f} {selftot[name] / 1000:12.1f} {maxd[name] / 1000:10.1f} {cnt[name]:8d}  {name}"
        )
        shown += 1
        if shown >= top:
            break


if __name__ == "__main__":
    main()
