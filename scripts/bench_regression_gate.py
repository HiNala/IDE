#!/usr/bin/env python3
"""Fail if Criterion median time regresses more than ALLOWED (default 10%) for one bench."""

from __future__ import annotations

import json
import os
import sys
from pathlib import Path

BENCH = "insert_random_coherent_500"
BASE_MAIN = "main"
BASE_PR = "pr"


def _median_ns(estimates_path: Path) -> float:
    data = json.loads(estimates_path.read_text(encoding="utf-8"))
    return float(data["median"]["point_estimate"])


def _find_estimates(root: Path, baseline: str) -> Path:
    matches = list(root.glob(f"**/{BENCH}/{baseline}/estimates.json"))
    if not matches:
        print(f"bench-regression: no {BENCH!r} / {baseline!r} estimates under {root}", file=sys.stderr)
        sys.exit(1)
    if len(matches) > 1:
        print(f"bench-regression: ambiguous estimates for {baseline}: {matches}", file=sys.stderr)
        sys.exit(1)
    return matches[0]


def main() -> None:
    root = Path(os.environ.get("CRITERION_ROOT", "target/criterion"))
    if not root.is_dir():
        print(f"bench-regression: missing {root}", file=sys.stderr)
        sys.exit(1)

    main_est = _find_estimates(root, BASE_MAIN)
    pr_est = _find_estimates(root, BASE_PR)
    m_main = _median_ns(main_est)
    m_pr = _median_ns(pr_est)
    allowed = float(os.environ.get("BENCH_REGRESSION_MAX", "0.10"))

    ratio = m_pr / m_main if m_main > 0 else float("nan")
    print(f"bench-regression: {BENCH} median main={m_main:g} pr={m_pr:g} ratio={ratio:.4f}")

    if m_pr > m_main * (1.0 + allowed):
        print(
            f"bench-regression: FAILED — PR median {m_pr:g} ns is > {(1.0 + allowed) * 100:.0f}% vs main {m_main:g} ns",
            file=sys.stderr,
        )
        sys.exit(1)
    print("bench-regression: OK")


if __name__ == "__main__":
    main()
