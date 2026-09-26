# Discovery fixture

This deliberately small repository is for demonstrating SecondEgo's read-only
issue discovery mode. It contains one swallowed exception and one public
function without a detected test link.

Run from the SecondEgo root:

```bash
make discover REPO=evaluation/discovery_fixture_repo
```

The command must not modify this directory.
