import sys
import time
from datetime import UTC, datetime
from pathlib import Path

from capacity import RESOURCES, forecast, rightsizing
from scorecard import load, render

LIMIT = (
    "Growth on this platform is synthetic, so the forecast demonstrates the method,"
    " not a real demand curve."
)
NAMES = {"cpuMillis": "cpu", "memoryMiB": "memory", "pods": "pods"}


def week(ts):
    d = datetime.fromtimestamp(ts, UTC).isocalendar()
    return f"{d.year}-W{d.week:02}"


def clock(ts):
    return f"{datetime.fromtimestamp(ts, UTC):%a %H:%M}"


def incidents_table(incidents):
    if not incidents:
        return ["None."]
    rows = [
        "| Tenant | Alert | Fired (UTC) | Detect | Recover | Fix |",
        "|---|---|---|---|---|---|",
    ]
    for i in incidents:
        detect = i.get("detectSeconds")
        resolved = i.get("resolvedAt")
        cells = [
            i["tenant"],
            i["alert"],
            clock(i["firedAt"]),
            "–" if detect is None else f"{detect} s",
            f"{resolved - i['firedAt']} s" if resolved else "not resolved",
            i.get("fix") or "–",
        ]
        rows.append(f"| {' | '.join(cells)} |")
    return rows


def capacity_section(usage):
    if not usage:
        return ["No usage snapshots yet."]
    last = usage[-1]
    used, cap = last["committed"], last["capacity"]
    shares = ", ".join(f"{NAMES[k]} {used[k] * 100 // cap[k]}%" for k in RESOURCES)
    days = forecast(usage)
    trend = "flat or falling" if days is None else f"reaches 85% in {days:.0f} days"
    sizing = [
        f"- `{name}` uses under a quarter of its {old} requests; `{new}` would fit."
        for name, old, new in rightsizing(last)
    ]
    return [
        f"Committed: {shares} of capacity (gate 85%).",
        f"Forecast over the last {min(len(usage), 14)} snapshots: {trend}. {LIMIT}",
        "",
        "### Rightsizing",
        "",
        *(sizing or ["No tenant sits far below its requests."]),
    ]


def report(name, runs, incidents, usage):
    runs = [r for r in runs if week(r["startedAt"]) == name]
    incidents = [i for i in incidents if week(i["firedAt"]) == name]
    return "\n".join(
        [
            f"# Weekly report {name}",
            "",
            "## Chaos runs",
            "",
            render(runs),
            "",
            "## Incidents",
            "",
            *incidents_table(incidents),
            "",
            "## Capacity",
            "",
            *capacity_section(usage),
            "",
        ]
    )


def main(data, out, name=""):
    name, d = name or week(time.time() - 7 * 86400), Path(data)
    runs, incidents = load(d / "runs.jsonl"), load(d / "incidents.jsonl")
    path = Path(out) / f"{name}.md"
    path.parent.mkdir(parents=True, exist_ok=True)
    text = report(name, runs, incidents, load(d / "usage.jsonl"))
    path.write_text(text, encoding="utf-8", newline="\n")
    print(path)


if __name__ == "__main__":
    main(*sys.argv[1:])
