import json
import sys
import time
import urllib.parse
import urllib.request
from statistics import linear_regression

from scorecard import load

GATE = 0.85
WINDOW = 14
IDLE = 0.25
RESOURCES = ("cpuMillis", "memoryMiB", "pods")
MEASURED = {"cpu": "cpuMillis", "memory": "memoryMiB"}
SELECT = 'namespace=~"tenant-.*", container!=""'
REQUESTS = "kube_pod_container_resource_requests"
QUERIES = {
    "cpuUsed": f"rate(container_cpu_usage_seconds_total{{{SELECT}}}[1h]) * 1000",
    "cpuRequested": f'{REQUESTS}{{{SELECT}, resource="cpu"}} * 1000',
    "memoryUsed": f"container_memory_working_set_bytes{{{SELECT}}} / 2^20",
    "memoryRequested": f'{REQUESTS}{{{SELECT}, resource="memory"}} / 2^20',
}


def query(prometheus, promql):
    q = urllib.parse.urlencode({"query": f"sum by (namespace) ({promql})"})
    with urllib.request.urlopen(f"{prometheus}/api/v1/query?{q}", timeout=30) as r:
        result = json.load(r)["data"]["result"]
    pairs = ((s["metric"]["namespace"], s["value"][1]) for s in result)
    return {ns.removeprefix("tenant-"): round(float(v)) for ns, v in pairs}


def snapshot(repo, measured, at):
    tiers, tenants = repo["tiers"], repo["tenants"]
    committed = {
        k: sum(tiers[t["tier"]]["quota"][k] for t in tenants) for k in RESOURCES
    }
    usage = {
        t["name"]: {"tier": t["tier"]}
        | {m: values.get(t["name"], 0) for m, values in measured.items()}
        for t in tenants
    }
    return {
        "at": at,
        "capacity": repo["capacity"],
        "committed": committed,
        "tiers": {name: tier["requests"] for name, tier in tiers.items()},
        "tenants": usage,
    }


def share(s):
    return max(s["committed"][k] / s["capacity"][k] for k in RESOURCES)


def forecast(snapshots):
    recent = snapshots[-WINDOW:]
    if len(recent) < 2:
        return None
    days = [s["at"] / 86400 for s in recent]
    slope, intercept = linear_regression(days, [share(s) for s in recent])
    if slope <= 0:
        return None
    return max(0.0, (GATE - intercept) / slope - days[-1])


def fits(usage, now, tier):
    return all(
        usage[f"{m}Used"] <= usage[f"{m}Requested"] * tier[k] / now[k]
        for m, k in MEASURED.items()
    )


def rightsizing(s):
    tiers = s["tiers"]
    by_memory = sorted(tiers, key=lambda n: tiers[n]["memoryMiB"])
    flagged = []
    for name, usage in s["tenants"].items():
        now = tiers[usage["tier"]]
        idle = all(usage[f"{m}Used"] < IDLE * usage[f"{m}Requested"] for m in MEASURED)
        smaller = by_memory[: by_memory.index(usage["tier"])]
        suggested = next((n for n in smaller if fits(usage, now, tiers[n])), None)
        if idle and suggested:
            flagged.append((name, usage["tier"], suggested))
    return flagged


def alarm(snapshots):
    days = forecast(snapshots)
    if days is None or days >= WINDOW:
        return ""
    return f"At the current trend, committed capacity reaches 85% in {days:.0f} days."


def main(command, *args):
    if command == "snapshot":
        prometheus, repo, path = args
        measured = {m: query(prometheus, q) for m, q in QUERIES.items()}
        repo = json.loads(open(repo, encoding="utf-8").read())
        with open(path, "a", encoding="utf-8", newline="\n") as f:
            f.write(json.dumps(snapshot(repo, measured, int(time.time()))) + "\n")
    elif command == "alarm":
        print(alarm(load(args[0])))


if __name__ == "__main__":
    main(*sys.argv[1:])
