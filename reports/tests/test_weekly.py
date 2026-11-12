from weekly import report, week

MONDAY = 1_762_732_800


def test_week_is_iso():
    assert week(MONDAY) == "2025-W46"
    assert week(MONDAY - 1) == "2025-W45"


def test_report_keeps_only_this_weeks_runs_and_incidents():
    runs = [
        {
            "experiment": "pod-kill",
            "phase": "done",
            "startedAt": MONDAY,
            "score": {"blastRadius": 0},
        },
        {
            "experiment": "pod-kill",
            "phase": "done",
            "startedAt": MONDAY - 1,
            "score": {"blastRadius": 2},
        },
    ]
    incidents = [
        {
            "tenant": "acme",
            "alert": "TenantAvailabilityFastBurn",
            "firedAt": MONDAY + 60,
            "resolvedAt": MONDAY + 360,
            "detectSeconds": 45,
            "fix": "Dry run.",
        },
    ]
    text = report("2025-W46", runs, incidents, [])
    assert "1 chaos runs: 1 scored" in text
    assert (
        "| acme | TenantAvailabilityFastBurn | Mon 00:01 | 45 s | 300 s | Dry run. |"
        in text
    )
    assert "No usage snapshots yet." in text


def test_report_states_the_forecasts_limit():
    usage = [
        {
            "at": 0,
            "capacity": {"cpuMillis": 10, "memoryMiB": 10, "pods": 10},
            "committed": {"cpuMillis": 5, "memoryMiB": 2, "pods": 1},
            "tiers": {},
            "tenants": {},
        }
    ]
    text = report("1970-W01", [], [], usage)
    assert "Committed: cpu 50%, memory 20%, pods 10% of capacity" in text
    assert "flat or falling" in text and "synthetic" in text
