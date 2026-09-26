# SecondEgo temporary-state garbage collection

SecondEgo creates temporary public-repository clones and detached Git attempt
worktrees outside the target repository. They are lifecycle-owned artifacts,
not user data and not a repository cache.

The collector is deliberately conservative:

1. It scans only the OS temp directory for `secondego-remote-*` and
   `secondego-attempt-*` directories.
2. New directories contain a `.secondego-owned` marker and an OS-held
   `.secondego-lock`.
3. A directory whose lock is held by an active run is never removed.
4. Symlinks and unmarked directories are never followed or deleted.
5. Only artifacts older than 24 hours are collected by default.
6. Scanning is bounded to 256 entries and size accounting to 2 GB per
   candidate pass.
7. Normal process completion removes its own clone/worktree immediately;
   garbage collection is the crash-recovery path.

The Rust runtime invokes collection before CLI and gateway startup. The Python
compatibility runtime does the same. A manual pass is available with:

```bash
make gc
```

For controlled testing, use `SECONDEGO_GC_RETENTION_SECONDS`,
`SECONDEGO_GC_MAX_ENTRIES`, and `SECONDEGO_GC_MAX_BYTES`. These values are
bounded by the implementation; they cannot turn collection into an arbitrary
filesystem delete operation.

The collector reports scanned, deleted, active, recent, unowned, error, and
reclaimed-byte counts. It never stores repository contents, credentials, or
model output.
