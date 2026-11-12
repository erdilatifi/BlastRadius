import json

from scorecard import main, render, rewrite


def run(experiment, phase="done", blast=0, invalid=None, detect=None, recover=30):
    score = {"blastRadius": None if invalid else blast, "invalid": invalid}
    return {
        "experiment": experiment,
        "phase": phase,
        "startedAt": 1_790_000_000,
        "detectSeconds": detect,
        "recoverSeconds": recover,
        "score": None if phase == "failed" else score,
    }


def test_render_counts_invalid_and_failed_runs_without_scoring_them():
    block = render(
        [
            run("pod-kill"),
            run("pod-kill", blast=1, recover=50),
            run("pod-kill", invalid="acme baseline error ratio 2.00%"),
            run("error-burst", phase="failed"),
            run("error-burst", detect=480),
        ]
    )
    assert "5 chaos runs: 3 scored, 1 invalid, 1 failed." in block
    assert "| `pod-kill` | 3 | 1 | 1 | – | 30 s |" in block
    assert "| `error-burst` | 2 | 0 | 0 | 480 s | 30 s |" in block


def test_empty_ledger_says_so():
    assert render([]) == "No chaos runs recorded yet."


def test_rewrite_replaces_only_the_generated_block():
    readme = "# Title\n<!-- scorecard:start -->\nold\n<!-- scorecard:end -->\nrest\n"
    assert rewrite(readme, "new") == (
        "# Title\n<!-- scorecard:start -->\nnew\n<!-- scorecard:end -->\nrest\n"
    )


def test_main_rewrites_the_readme_from_the_ledger(tmp_path):
    ledger, readme = tmp_path / "runs.jsonl", tmp_path / "README.md"
    ledger.write_text(json.dumps(run("cpu-stress")) + "\n")
    readme.write_text("<!-- scorecard:start -->\n<!-- scorecard:end -->\n")
    main(ledger, readme)
    text = readme.read_text(encoding="utf-8")
    assert "| `cpu-stress` | 1 | 0 | 0 | – | 30 s |" in text
