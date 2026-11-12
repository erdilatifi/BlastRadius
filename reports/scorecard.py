import json
import sys
from collections import defaultdict
from datetime import UTC, datetime
from pathlib import Path
from statistics import median

START, END = "<!-- scorecard:start -->", "<!-- scorecard:end -->"


def load(path):
    p = Path(path)
    lines = p.read_text(encoding="utf-8").splitlines() if p.exists() else []
    return [json.loads(line) for line in lines if line.strip()]


def radius(run):
    return (run.get("score") or {}).get("blastRadius")


def seconds(values):
    values = [v for v in values if v is not None]
    return f"{median(values):.0f} s" if values else "–"


def render(runs):
    if not runs:
        return "No chaos runs recorded yet."
    scored = [r for r in runs if radius(r) is not None]
    failed = sum(r["phase"] == "failed" for r in runs)
    invalid = len(runs) - len(scored) - failed
    last = datetime.fromtimestamp(max(r["startedAt"] for r in runs), UTC)
    lines = [
        f"{len(runs)} chaos runs: {len(scored)} scored, {invalid} invalid, "
        f"{failed} failed. Last run {last:%Y-%m-%d %H:%M} UTC.",
        "",
        "| Experiment | Runs | Runs with impacted bystanders | Most impacted"
        " | Detect (median) | Recover (median) |",
        "|---|---|---|---|---|---|",
    ]
    by = defaultdict(list)
    for r in runs:
        by[r["experiment"]].append(r)
    for name, rs in sorted(by.items()):
        radii = [radius(r) for r in rs if radius(r) is not None]
        lines.append(
            f"| `{name}` | {len(rs)} | {sum(x > 0 for x in radii)} "
            f"| {max(radii, default='–')} "
            f"| {seconds(r.get('detectSeconds') for r in rs)} "
            f"| {seconds(r.get('recoverSeconds') for r in rs)} |"
        )
    return "\n".join(lines)


def rewrite(readme, block):
    head, rest = readme.split(START, 1)
    _, tail = rest.split(END, 1)
    return f"{head}{START}\n{block}\n{END}{tail}"


def main(ledger, readme):
    p = Path(readme)
    text = rewrite(p.read_text(encoding="utf-8"), render(load(ledger)))
    p.write_text(text, encoding="utf-8", newline="\n")


if __name__ == "__main__":
    main(*sys.argv[1:])
