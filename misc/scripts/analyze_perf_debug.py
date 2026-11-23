"""Analyze `PerfDebug` logs produced by the `debug_perf` cargo feature.

Groups `PerfDebug: func(...), time_us(...)` records by frame, splits the run into
initialization (the first `--init-frames` frames) and runtime, then prints the
functions with the highest average time per frame and the highest single-frame
time. Recorded named counters (`name(value)` fields) are printed below each
function.

Usage:
    python3 analyze_perf_debug.py <log> [--init-frames N]
                                  [--top-avg N] [--top-max N]
"""

import argparse
import re
from collections import defaultdict
from dataclasses import dataclass, field

FRAME_RE = re.compile(r"PerfDebug: frame\((\d+)\)")
FUNC_RE = re.compile(r"PerfDebug: func\(([^)]+)\), time_us\((\d+)\)(.*)$")
TAG_RE = re.compile(r"([A-Za-z_][A-Za-z0-9_]*)\((\d+)\)")


@dataclass
class TagStats:
    total: int = 0
    count: int = 0
    max: int = 0

    def add(self, value: int) -> None:
        self.total += value
        self.count += 1
        self.max = max(self.max, value)

    def avg(self) -> float:
        if not self.count:
            return 0.0
        return self.total / self.count


@dataclass
class Stats:
    total_us: int = 0
    count: int = 0
    max_us: int = 0
    tags: dict[str, TagStats] = field(default_factory=dict)

    def add(self, time_us: int, tags: list[tuple[str, int]]) -> None:
        self.total_us += time_us
        self.count += 1
        self.max_us = max(self.max_us, time_us)
        for name, value in tags:
            self.tags.setdefault(name, TagStats()).add(value)


@dataclass
class Phase:
    name: str
    frames: set[int]
    functions: dict[str, Stats]

    def avg_us(self, name: str) -> float:
        if not self.frames:
            return 0.0
        return self.functions[name].total_us / len(self.frames)


def parse_log(path: str, init_frames: int) -> tuple[Phase, Phase]:
    init = Phase("Initialization", set(), defaultdict(Stats))
    runtime = Phase("Runtime", set(), defaultdict(Stats))

    def phase_for(frame: int) -> Phase:
        return init if frame <= init_frames else runtime

    current_frame: int | None = None
    with open(path, encoding="utf-8", errors="replace") as log:
        for line in log:
            frame_match = FRAME_RE.search(line)
            if frame_match:
                current_frame = int(frame_match.group(1))
                phase_for(current_frame).frames.add(current_frame)
                continue

            func_match = FUNC_RE.search(line)
            if not func_match:
                continue
            if current_frame is None:
                continue
            name, time_us = func_match.group(1), int(func_match.group(2))
            tags = [(tag, int(value)) for tag, value in TAG_RE.findall(func_match.group(3))]
            phase_for(current_frame).functions[name].add(time_us, tags)

    return init, runtime


def format_tags(stats: Stats) -> str:
    parts = []
    for name in sorted(stats.tags):
        tag = stats.tags[name]
        parts.append(f"{name}({tag.avg():.1f}/{tag.max})")
    return ", ".join(parts)


def print_phase(phase: Phase, top_avg: int, top_max: int) -> None:
    print()
    print(f"=== {phase.name}: frames {len(phase.frames)} ===")
    if not phase.frames or not phase.functions:
        print("no data")
        return

    by_avg = sorted(
        phase.functions,
        key=lambda name: (phase.avg_us(name), phase.functions[name].total_us),
        reverse=True,
    )
    print()
    print(f"Top {top_avg} by average time per frame:")
    print(f"{'avg_us':>10} {'total_us':>12} {'max_us':>10} {'calls':>8}  function [counters(avg/max)]")
    for name in by_avg[:top_avg]:
        stats = phase.functions[name]
        tags = format_tags(stats)
        print(
            f"{phase.avg_us(name):10.1f} {stats.total_us:12d} {stats.max_us:10d} "
            f"{stats.count:8d}  {name}",
            end="" if tags else "\n"
        )
        if tags:
            print(f" counters: {tags}")

    by_max = sorted(
        phase.functions,
        key=lambda name: (phase.functions[name].max_us, phase.functions[name].total_us),
        reverse=True,
    )
    print()
    print(f"Top {top_max} by maximum time in a single frame:")
    print(f"{'max_us':>10} {'avg_us':>10} {'calls':>8}  function")
    for name in by_max[:top_max]:
        stats = phase.functions[name]
        tags = format_tags(stats)
        print(
            f"{stats.max_us:10d} {phase.avg_us(name):10.1f} {stats.count:8d}  {name}",
            end="" if tags else "\n"
        )
        if tags:
            print(f" counters: {tags}")


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("log", help="path to the PerfDebug log")
    parser.add_argument(
        "--init-frames",
        type=int,
        default=200,
        help="number of leading frames treated as initialization (default: 200)",
    )
    parser.add_argument("--top-avg", type=int, default=30)
    parser.add_argument("--top-max", type=int, default=10)
    args = parser.parse_args()

    init, runtime = parse_log(args.log, args.init_frames)
    print_phase(init, args.top_avg, args.top_max)
    print_phase(runtime, args.top_avg, args.top_max)


if __name__ == "__main__":
    main()
