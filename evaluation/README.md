# Local evaluation rehearsal

The harness repository is not the repository being fixed. Use the fixture below as
a separate target, or point `make run` at a cloned evaluation repository.

## Rehearse the deterministic path

From the repository root:

```bash
TARGET_DIR="$(mktemp -d)/pagination-fixture"
cp -R evaluation/fixture_repo/. "$TARGET_DIR"
git -C "$TARGET_DIR" init -q
git -C "$TARGET_DIR" config user.email evaluation@example.com
git -C "$TARGET_DIR" config user.name "SecondEgo Evaluation"
git -C "$TARGET_DIR" add .
git -C "$TARGET_DIR" -c commit.gpgSign=false commit -qm baseline
PATH="$(pwd)/.venv/bin:$PATH" .venv/bin/secondego solve \
  --repo "$TARGET_DIR" \
  --issue "Fix pagination so page 1 returns the first page and page 2 returns the second page." \
  --plan evaluation/fixture_plan.json
git -C "$TARGET_DIR" diff
```

The target must be a clean Git repository with an initial commit because the runtime
uses a detached worktree for each attempt. A failed attempt is discarded; a passing
attempt is transferred back to the target.

## Rehearse the live evaluator path

```bash
set -a
source .env
set +a
make run
```

When prompted, enter the target repository path and the issue. Do not enter the
SecondEgo harness repository itself; the evaluator supplies a separate target.

Before and after each rehearsal, inspect:

```bash
git -C "$TARGET_DIR" status --short
git -C "$TARGET_DIR" diff --stat
```

The run should solve the issue, keep tests unchanged, and leave a small explainable
diff. `make test` remains the harness regression suite; it is not the official live
evaluation.
