from capacity import alarm, forecast, rightsizing, snapshot

TIERS = {
    "small": {
        "requests": {"cpuMillis": 50, "memoryMiB": 32},
        "quota": {"cpuMillis": 400, "memoryMiB": 256, "pods": 6},
    },
    "large": {
        "requests": {"cpuMillis": 200, "memoryMiB": 128},
        "quota": {"cpuMillis": 1600, "memoryMiB": 1024, "pods": 12},
    },
}
REPO = {
    "tiers": TIERS,
    "capacity": {"cpuMillis": 4000, "memoryMiB": 4096, "pods": 60},
    "tenants": [{"name": "acme", "tier": "small"}, {"name": "globex", "tier": "large"}],
}
DAY = 86400


def measured(cpu, memory):
    return {
        "cpuUsed": {"globex": cpu},
        "cpuRequested": {"globex": 400},
        "memoryUsed": {"globex": memory},
        "memoryRequested": {"globex": 256},
    }


def growing(per_day, days=14):
    return [
        {
            "at": d * DAY,
            "capacity": {"cpuMillis": 100, "memoryMiB": 100, "pods": 100},
            "committed": {"cpuMillis": 10 + per_day * d, "memoryMiB": 0, "pods": 0},
        }
        for d in range(days)
    ]


def test_snapshot_commits_each_tenants_quota():
    s = snapshot(REPO, measured(10, 10), 0)
    assert s["committed"] == {"cpuMillis": 2000, "memoryMiB": 1280, "pods": 18}
    assert s["tenants"]["acme"] == {
        "tier": "small",
        "cpuUsed": 0,
        "cpuRequested": 0,
        "memoryUsed": 0,
        "memoryRequested": 0,
    }


def test_forecast_projects_the_linear_trend_to_the_gate():
    assert forecast(growing(5)) == 2.0
    assert forecast(growing(0)) is None
    assert forecast(growing(5, days=1)) is None
    assert (
        alarm(growing(5))
        == "At the current trend, committed capacity reaches 85% in 2 days."
    )
    assert alarm(growing(1)) == ""


def test_rightsizing_suggests_a_smaller_tier_only_when_usage_fits_it():
    assert rightsizing(snapshot(REPO, measured(40, 20), 0)) == [
        ("globex", "large", "small")
    ]
    assert rightsizing(snapshot(REPO, measured(150, 20), 0)) == []
    assert rightsizing(snapshot(REPO, measured(40, 200), 0)) == []
