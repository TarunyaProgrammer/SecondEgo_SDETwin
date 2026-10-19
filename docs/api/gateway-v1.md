# SecondEgo local gateway API

The desktop shell talks to the Rust gateway over loopback HTTP. The gateway is
not a public service: it binds to `127.0.0.1`, requires the per-launch
`X-SecondEgo-Token` header, and must not be exposed through a proxy or LAN
listener without a separate authentication design.

## Common rules

- `Content-Type: application/json` is required for request bodies.
- `X-SecondEgo-Token` is required on every API route.
- Request bodies are bounded; oversized tasks are rejected before execution.
- Provider errors are classified and sanitized. Raw provider response bodies,
  prompts, and keys are not returned.
- Engine events use `schema_version: 1`.
- Diff payloads use `format: "git-unified-v1"` and are bounded by the gateway.

## Health

```http
GET /api/health
X-SecondEgo-Token: <launch token>
```

```json
{"ok":true,"api_version":1,"engine":"rust"}
```

## Provider connection test

```http
POST /api/providers/test
X-SecondEgo-Token: <launch token>
```

The body is empty. The gateway sends a fixed structured-output probe through
the selected provider and returns only safe status data:

```json
{"ok":true,"latency_ms":842}
```

Failures return `502` with a bounded classified error string. This endpoint is
not a generic prompt proxy.

## Start a run

```http
POST /api/runs
X-SecondEgo-Token: <launch token>
Content-Type: application/json

{"repository":"/absolute/path/to/repository","issue":"Fix pagination and add verification"}
```

The response is asynchronous:

```json
{"request_id":"<uuid>","status":"QUEUED"}
```

`mode: "discover"` starts a read-only repository scan. Discovery accepts
bounded `lenses` and `max_findings` fields.

## Read a run

```http
GET /api/runs/<uuid>?offset=0
X-SecondEgo-Token: <launch token>
```

The response includes the next event slice, current status, safe error data,
and a result after termination. A completed result contains normalized diff
data:

```json
{
  "changed_paths":["src/pagination.py"],
  "diff_transferred":true,
  "diff":{
    "format":"git-unified-v1",
    "truncated":false,
    "patch":"diff --git ...",
    "files":[{
      "path":"src/pagination.py",
      "status":"modified",
      "additions":1,
      "deletions":1,
      "hunks":[{"header":"@@ -10,1 +10,1 @@","lines":[{"kind":"delete","content":"old"},{"kind":"add","content":"new"}]}]
    }]
  },
  "verification":{"passed":true}
}
```

The UI should render `diff.files` as the primary model and use `diff.patch` as
the copy/export representation. It must not parse terminal output to recreate
file state.

## Cancel a run

```http
POST /api/runs/<uuid>/cancel
X-SecondEgo-Token: <launch token>
```

Cancellation is cooperative. The terminal result must be `CANCELLED` or a
safe terminal failure; partial worktree changes are not transferred.

## Next version boundary

Gateway v1 is a local desktop contract, not a hosted multi-user API. Before a
remote service is introduced, v2 must add explicit tenant authentication,
workspace ownership, durable run storage, event replay cursors, quotas, and a
separate secret-management boundary.
