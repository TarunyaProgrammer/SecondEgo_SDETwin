PYTHON ?= python3
VENV ?= .venv
UI_MODE ?= headless
ENGINE ?= rust
RUST_BIN ?= $(CURDIR)/engine-rs/target/release/secondego-cli

.PHONY: setup run run-python desktop desktop-electron desktop-macos rust-check rust-test rust-build rust-build-release rust-run test clean

setup:
	$(PYTHON) -m venv $(VENV)
	$(VENV)/bin/pip install 'setuptools>=68' 'pytest>=8.0' 'google-genai>=1.0'
	$(VENV)/bin/pip install --no-build-isolation -e .
	cargo build --manifest-path engine-rs/Cargo.toml --release -p secondego-cli -p secondego-gateway

run:
	@if [ "$(ENGINE)" = "python" ]; then \
		test -n "$$AI_API_KEY" || (echo "AI_API_KEY must be set for evaluation"; exit 2); \
		SECONDEGO_UI_MODE="$(UI_MODE)" PATH="$(CURDIR)/$(VENV)/bin:$$PATH" $(VENV)/bin/secondego-tui; \
		code=$$?; \
	else \
		$(MAKE) rust-build-release || exit $$?; \
		"$(RUST_BIN)" --interactive $(if $(filter events,$(UI_MODE)),--ui events,); \
		code=$$?; \
	fi; \
	if [ "$$code" -eq 130 ]; then exit 0; fi; \
	exit "$$code"

run-python:
	@test -n "$$AI_API_KEY" || (echo "AI_API_KEY must be set for evaluation"; exit 2)
	SECONDEGO_UI_MODE="$(UI_MODE)" PATH="$(CURDIR)/$(VENV)/bin:$$PATH" $(VENV)/bin/secondego-tui

desktop:
	PATH="$(CURDIR)/$(VENV)/bin:$$PATH" $(VENV)/bin/secondego-desktop

desktop-electron: rust-build
	@if [ "$(shell uname -s)" = "Darwin" ]; then \
		$(MAKE) desktop-macos; \
	else \
		npm --prefix apps/desktop run desktop; \
	fi

desktop-macos:
	npm --prefix apps/desktop run build
	mkdir -p apps/desktop/native/.build/SecondEgo.app/Contents/MacOS
	swiftc -target "$(shell uname -m)-apple-macosx11.0" -framework Cocoa -framework WebKit -O -o apps/desktop/native/.build/SecondEgo.app/Contents/MacOS/SecondEgo apps/desktop/native/SecondEgo.swift
	cp apps/desktop/native/Info.plist apps/desktop/native/.build/SecondEgo.app/Contents/Info.plist
	@port="$${SECONDEGO_GATEWAY_PORT:-$$(python3 -c 'import socket; s=socket.socket(); s.bind(("127.0.0.1", 0)); print(s.getsockname()[1]); s.close()')}"; \
	cleanup() { pid="$$(lsof -tiTCP:"$$port" -sTCP:LISTEN 2>/dev/null | head -1)"; if [ -n "$$pid" ]; then kill "$$pid" 2>/dev/null || true; fi; }; \
	on_signal() { cleanup; exit 130; }; \
	trap cleanup EXIT; trap on_signal INT TERM; \
	SECONDEGO_ROOT="$(CURDIR)" SECONDEGO_GATEWAY_PORT="$$port" apps/desktop/native/.build/SecondEgo.app/Contents/MacOS/SecondEgo; \
	code=$$?; \
	if [ "$$code" -eq 130 ]; then exit 0; fi; \
	exit "$$code"

rust-check:
	cargo check --manifest-path engine-rs/Cargo.toml

rust-test:
	cargo test --manifest-path engine-rs/Cargo.toml

rust-build:
	cargo build --manifest-path engine-rs/Cargo.toml

rust-build-release:
	cargo build --manifest-path engine-rs/Cargo.toml --release -p secondego-cli -p secondego-gateway

rust-run:
	cargo run --manifest-path engine-rs/Cargo.toml -p secondego-cli -- --workspace "$(CURDIR)" --task "$(TASK)" $(if $(SCRIPT),--script "$(SCRIPT)",)

test:
	$(VENV)/bin/python -m pytest -q

clean:
	rm -rf $(VENV) .pytest_cache build dist src/secondego.egg-info
	find src tests -type d -name '__pycache__' -prune -exec rm -rf {} +
