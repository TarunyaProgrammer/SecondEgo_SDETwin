from __future__ import annotations

import argparse
import asyncio
import json
import mimetypes
import os
import secrets
import threading
from dataclasses import dataclass, field
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
from pathlib import Path
from typing import Any
from urllib.parse import parse_qs, quote, urlparse
from uuid import uuid4

from SecondEgo.app import build_engine, build_gemini_planner, build_resources
from SecondEgo.config import configured_model
from SecondEgo.core.events import EngineEvent
from SecondEgo.core.state import AcceptanceCriterion
from SecondEgo.repository.source import RepositorySourceError, resolve_repository, validate_repository_source
from SecondEgo.storage.redaction import redact_sensitive


MAX_BODY_BYTES = 32_768
MAX_ISSUE_CHARS = 12_000
MAX_REPOSITORY_CHARS = 4_096
MAX_MODEL_CHARS = 256


@dataclass
class RunRecord:
    request_id: str
    repository: str
    issue: str
    model: str
    source_repository: str | None = None
    status: str = "QUEUED"
    events: list[dict[str, Any]] = field(default_factory=list)
    result: dict[str, Any] | None = None
    error: str | None = None
    lock: threading.Lock = field(default_factory=threading.Lock, repr=False)

    def add_event(self, event: EngineEvent) -> None:
        with self.lock:
            self.events.append(event.to_dict())
            if event.event_type == "run.terminated":
                self.status = event.status or "FAILED"
            elif self.status == "QUEUED":
                self.status = "RUNNING"

    def snapshot(self, *, event_offset: int = 0) -> dict[str, Any]:
        with self.lock:
            events = list(self.events[max(event_offset, 0) :])
            return {
                "request_id": self.request_id,
                "repository": self.repository,
                "source_repository": self.source_repository,
                "issue": self.issue,
                "model": self.model,
                "status": self.status,
                "events": events,
                "result": self.result,
                "error": self.error,
            }


class RunRegistry:
    """Thread-safe run registry used only by the optional local observer."""

    def __init__(self) -> None:
        self._runs: dict[str, RunRecord] = {}
        self._lock = threading.Lock()

    def submit(self, *, repository: str, issue: str, model: str | None = None) -> RunRecord:
        repository_value = repository.strip()
        issue_value = issue.strip()
        if not repository_value or len(repository_value) > MAX_REPOSITORY_CHARS:
            raise ValueError("repository must be a non-empty path or HTTPS GitHub URL within the request limit")
        try:
            validate_repository_source(repository_value)
        except RepositorySourceError as exc:
            raise ValueError(str(exc)) from exc
        if not issue_value or len(issue_value) > MAX_ISSUE_CHARS:
            raise ValueError("issue must be non-empty and within the request limit")
        model_value = (model or configured_model()).strip()
        if len(model_value) > MAX_MODEL_CHARS:
            raise ValueError("model identifier exceeds the request limit")

        record = RunRecord(
            request_id=str(uuid4()),
            repository=repository_value,
            issue=issue_value,
            model=model_value or configured_model(),
            source_repository=repository_value,
        )
        with self._lock:
            self._runs[record.request_id] = record
        threading.Thread(target=self._run, args=(record,), daemon=True).start()
        return record

    def get(self, request_id: str) -> RunRecord | None:
        with self._lock:
            return self._runs.get(request_id)

    def _run(self, record: RunRecord) -> None:
        try:
            resolved = resolve_repository(record.repository)
            with record.lock:
                record.repository = str(resolved.root)
            engine = build_engine(
                resolved.root,
                resources=build_resources(),
                event_sink=record.add_event,
            )
            result = asyncio.run(
                engine.run_with_planner(
                    task=record.issue,
                    acceptance_criteria=(
                        AcceptanceCriterion("Implement and verify the supplied issue"),
                    ),
                    planner=build_gemini_planner(engine.resources, record.model),
                )
            )
            with record.lock:
                record.result = _result_payload(result)
                record.status = result.state.status.value
        except Exception as exc:
            # Keep gateway failures bounded and free of provider URLs or secrets.
            with record.lock:
                record.status = "FAILED"
                record.error = f"{type(exc).__name__}: {redact_sensitive(str(exc))[:500]}"


def _result_payload(result: Any) -> dict[str, Any]:
    verification = result.verification
    failure = verification.failure_record
    return {
        "run_id": result.state.run_id,
        "status": result.state.status.value,
        "phase": result.state.phase.value,
        "termination_reason": redact_sensitive(result.state.termination_reason or ""),
        "changed_paths": sorted(result.state.changed_paths),
        "resource_usage": result.state.resource_usage,
        "verification": {
            "passed": verification.passed,
            "failure_class": verification.failure_class.value,
            "failure_summary": redact_sensitive(verification.failure_summary),
            "commands": list(verification.commands),
            "failure_record": None
            if failure is None
            else {
                "failure_class": failure.failure_class.value,
                "summary": redact_sensitive(failure.summary),
                "failing_tests": list(failure.failing_tests),
                "error_locations": list(failure.error_locations),
            },
        },
        "evidence": [
            {
                "reference": item.reference,
                "summary": redact_sensitive(item.summary),
                "source": item.source,
                "importance": item.importance,
            }
            for item in result.evidence
        ],
    }


class GatewayHandler(BaseHTTPRequestHandler):
    server_version = "SecondEgoGateway/1"

    @property
    def gateway(self) -> "DesktopGateway":
        return self.server.gateway  # type: ignore[attr-defined]

    def do_GET(self) -> None:  # noqa: N802
        parsed = urlparse(self.path)
        if parsed.path == "/":
            if self.gateway.react_build_available and not parsed.query:
                self._send_redirect(f"/?token={quote(self.gateway.token)}")
                return
            self._send_html(self.gateway.dashboard_html())
            return
        if self.gateway.react_build_available and parsed.path.startswith("/assets/"):
            self._send_static_asset(parsed.path)
            return
        if not self._authorized():
            self._send_json({"error": "unauthorized"}, status=401)
            return
        if parsed.path == "/api/health":
            self._send_json({"ok": True, "api_version": 1})
            return
        if parsed.path.startswith("/api/runs/"):
            request_id = parsed.path.removeprefix("/api/runs/").split("/", 1)[0]
            record = self.gateway.registry.get(request_id)
            if record is None:
                self._send_json({"error": "run not found"}, status=404)
                return
            query = parse_qs(parsed.query)
            try:
                offset = int(query.get("offset", ["0"])[0])
            except ValueError:
                offset = 0
            self._send_json(record.snapshot(event_offset=offset))
            return
        self._send_json({"error": "not found"}, status=404)

    def do_POST(self) -> None:  # noqa: N802
        if not self._authorized():
            self._send_json({"error": "unauthorized"}, status=401)
            return
        if self.path != "/api/runs":
            self._send_json({"error": "not found"}, status=404)
            return
        length = self.headers.get("Content-Length")
        try:
            size = int(length or "-1")
        except ValueError:
            size = -1
        if size < 0 or size > MAX_BODY_BYTES:
            self._send_json({"error": "request body too large or missing"}, status=413)
            return
        try:
            payload = json.loads(self.rfile.read(size))
            if not isinstance(payload, dict):
                raise ValueError("request body must be a JSON object")
            record = self.gateway.registry.submit(
                repository=str(payload.get("repository", "")),
                issue=str(payload.get("issue", "")),
                model=str(payload.get("model", "")) or None,
            )
        except (ValueError, json.JSONDecodeError) as exc:
            self._send_json({"error": str(exc)}, status=400)
            return
        self._send_json(
            {"request_id": record.request_id, "status": record.status}, status=202
        )

    def do_OPTIONS(self) -> None:  # noqa: N802
        # Browser preflight requests do not carry the application token. The
        # actual GET/POST remains authenticated in do_GET/do_POST.
        self.send_response(204)
        self._send_cors_headers()
        self.send_header("Content-Length", "0")
        self.end_headers()

    def _authorized(self) -> bool:
        return secrets.compare_digest(
            self.headers.get("X-SecondEgo-Token", ""), self.gateway.token
        )

    def _send_json(self, value: object, *, status: int = 200) -> None:
        body = json.dumps(value, separators=(",", ":"), default=str).encode("utf-8")
        self.send_response(status)
        self.send_header("Content-Type", "application/json")
        self._send_cors_headers()
        self.send_header("Content-Length", str(len(body)))
        self.end_headers()
        self.wfile.write(body)

    def _send_html(self, body: str) -> None:
        encoded = body.encode("utf-8")
        self.send_response(200)
        self.send_header("Content-Type", "text/html; charset=utf-8")
        self._send_cors_headers()
        self.send_header("Content-Length", str(len(encoded)))
        self.end_headers()
        self.wfile.write(encoded)

    def _send_redirect(self, location: str) -> None:
        self.send_response(302)
        self.send_header("Location", location)
        self._send_cors_headers()
        self.send_header("Content-Length", "0")
        self.end_headers()

    def _send_static_asset(self, request_path: str) -> None:
        relative = request_path.removeprefix("/assets/")
        candidate = (self.gateway.react_dist / "assets" / relative).resolve()
        assets_root = (self.gateway.react_dist / "assets").resolve()
        if assets_root not in candidate.parents or not candidate.is_file():
            self._send_json({"error": "asset not found"}, status=404)
            return
        body = candidate.read_bytes()
        self.send_response(200)
        self.send_header("Content-Type", mimetypes.guess_type(candidate.name)[0] or "application/octet-stream")
        self._send_cors_headers()
        self.send_header("Content-Length", str(len(body)))
        self.end_headers()
        self.wfile.write(body)

    def log_message(self, format: str, *args: object) -> None:
        return

    def _send_cors_headers(self) -> None:
        self.send_header("Access-Control-Allow-Origin", "*")
        self.send_header("Access-Control-Allow-Methods", "GET, POST, OPTIONS")
        self.send_header("Access-Control-Allow-Headers", "Content-Type, X-SecondEgo-Token")


class DesktopGateway:
    def __init__(self, *, host: str = "127.0.0.1", port: int = 8787) -> None:
        self.token = os.environ.get("SECONDEGO_UI_TOKEN", "").strip() or secrets.token_urlsafe(24)
        self.registry = RunRegistry()
        self.server = ThreadingHTTPServer((host, port), GatewayHandler)
        self.server.gateway = self  # type: ignore[attr-defined]

    @property
    def react_dist(self) -> Path:
        return Path(__file__).resolve().parents[3] / "apps" / "desktop" / "dist"

    @property
    def react_build_available(self) -> bool:
        return (self.react_dist / "index.html").is_file()

    @property
    def address(self) -> tuple[str, int]:
        return self.server.server_address[0], self.server.server_address[1]

    def serve_forever(self) -> None:
        host, port = self.address
        print(f"SecondEgo UI: http://{host}:{port}/", flush=True)
        print(f"SecondEgo UI token: {self.token}", flush=True)
        print("The UI is optional and observational; the Python engine remains authoritative.", flush=True)
        try:
            self.server.serve_forever()
        except KeyboardInterrupt:
            pass
        finally:
            self.server.server_close()

    def dashboard_html(self) -> str:
        if self.react_build_available:
            return (self.react_dist / "index.html").read_text(encoding="utf-8")
        static_path = Path(__file__).with_name("static") / "index.html"
        html = static_path.read_text(encoding="utf-8")
        return html.replace("__SECOND_EGO_TOKEN__", self.token)


def main() -> int:
    parser = argparse.ArgumentParser(description="Run the optional SecondEgo local observer UI.")
    parser.add_argument("--host", default="127.0.0.1", help="bind address; loopback is recommended")
    parser.add_argument("--port", default=8787, type=int)
    arguments = parser.parse_args()
    if arguments.host not in {"127.0.0.1", "localhost", "::1"}:
        parser.error("the desktop gateway must bind to loopback")
    DesktopGateway(host=arguments.host, port=arguments.port).serve_forever()
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
