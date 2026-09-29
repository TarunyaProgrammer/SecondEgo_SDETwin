PYTHON ?= python3
VENV ?= .venv
UI_MODE ?= headless
UI ?= off
ENGINE ?= rust
PROFILE ?=
NONINTERACTIVE ?= 0
CONFIGURE ?= 0
RUST_BIN ?= $(CURDIR)/engine-rs/target/release/secondego-cli
RUST_MANIFEST ?= engine-rs/Cargo.toml
DESKTOP_DIR ?= apps/desktop

# Load only recognised, missing values from an ignored local .env file. This
# preserves an evaluator's exported AI_API_KEY/provider/model and never executes
# dotenv content as shell code.
define LOAD_LOCAL_ENV
if [ -f ".env" ]; then \
	while IFS= read -r dotenv_line || [ -n "$$dotenv_line" ]; do \
		case "$$dotenv_line" in \
			AI_API_KEY=*) \
				if [ -z "$${AI_API_KEY:-}" ]; then AI_API_KEY=$${dotenv_line#AI_API_KEY=}; export AI_API_KEY; fi ;; \
			GROQ_API_KEY=*) \
				if [ -z "$${GROQ_API_KEY:-}" ]; then GROQ_API_KEY=$${dotenv_line#GROQ_API_KEY=}; export GROQ_API_KEY; fi ;; \
			SECONDEGO_PROVIDER=*) \
				if [ -z "$${SECONDEGO_PROVIDER:-}" ]; then SECONDEGO_PROVIDER=$${dotenv_line#SECONDEGO_PROVIDER=}; export SECONDEGO_PROVIDER; fi ;; \
			SECONDEGO_MODEL=*) \
				if [ -z "$${SECONDEGO_MODEL:-}" ]; then SECONDEGO_MODEL=$${dotenv_line#SECONDEGO_MODEL=}; export SECONDEGO_MODEL; fi ;; \
			VOICE_ENABLED=*) \
				if [ -z "$${VOICE_ENABLED:-}" ]; then VOICE_ENABLED=$${dotenv_line#VOICE_ENABLED=}; export VOICE_ENABLED; fi ;; \
			GEMINI_API_KEY=*) \
				if [ -z "$${GEMINI_API_KEY:-}" ]; then GEMINI_API_KEY=$${dotenv_line#GEMINI_API_KEY=}; export GEMINI_API_KEY; fi ;; \
			GEMINI_TTS_MODEL=*) \
				if [ -z "$${GEMINI_TTS_MODEL:-}" ]; then GEMINI_TTS_MODEL=$${dotenv_line#GEMINI_TTS_MODEL=}; export GEMINI_TTS_MODEL; fi ;; \
			VOICE_TIMEOUT=*) \
				if [ -z "$${VOICE_TIMEOUT:-}" ]; then VOICE_TIMEOUT=$${dotenv_line#VOICE_TIMEOUT=}; export VOICE_TIMEOUT; fi ;; \
			VOICE_MAX_QUEUE_SIZE=*) \
				if [ -z "$${VOICE_MAX_QUEUE_SIZE:-}" ]; then VOICE_MAX_QUEUE_SIZE=$${dotenv_line#VOICE_MAX_QUEUE_SIZE=}; export VOICE_MAX_QUEUE_SIZE; fi ;; \
			SECONDEGO_PROFILE=*) \
				if [ -z "$${SECONDEGO_PROFILE:-}" ]; then SECONDEGO_PROFILE=$${dotenv_line#SECONDEGO_PROFILE=}; export SECONDEGO_PROFILE; fi ;; \
			SECONDEGO_SURFACE=*) \
				if [ -z "$${SECONDEGO_SURFACE:-}" ]; then SECONDEGO_SURFACE=$${dotenv_line#SECONDEGO_SURFACE=}; export SECONDEGO_SURFACE; fi ;; \
			SECONDEGO_TERMINAL_MODE=*) \
				if [ -z "$${SECONDEGO_TERMINAL_MODE:-}" ]; then SECONDEGO_TERMINAL_MODE=$${dotenv_line#SECONDEGO_TERMINAL_MODE=}; export SECONDEGO_TERMINAL_MODE; fi ;; \
			SECONDEGO_GESTURES_ENABLED=*) \
				if [ -z "$${SECONDEGO_GESTURES_ENABLED:-}" ]; then SECONDEGO_GESTURES_ENABLED=$${dotenv_line#SECONDEGO_GESTURES_ENABLED=}; export SECONDEGO_GESTURES_ENABLED; fi ;; \
		esac; \
	done < ".env"; \
fi;
endef

.PHONY: setup setup-python setup-desktop check-git check-rust check-python check-node config run run-rust run-python discover help ui desktop desktop-electron desktop-macos judge rust-check rust-test rust-build rust-build-release rust-run gc test clean

check-git:
	@command -v git >/dev/null 2>&1 || { echo "Error: git is required. Install Git, then re-run make setup."; exit 2; }

check-rust:
	@command -v cargo >/dev/null 2>&1 || { echo "Error: Cargo/Rust 1.85+ is required for the evaluator harness. Install Rust with rustup, then re-run make setup."; exit 2; }

check-python:
	@command -v "$(PYTHON)" >/dev/null 2>&1 || { echo "Error: Python 3.12+ is required for make test or the optional Python compatibility shell."; exit 2; }
	@$(PYTHON) -c 'import sys; raise SystemExit("Error: Python 3.12+ is required." if sys.version_info < (3, 12) else 0)'

check-node:
	@command -v node >/dev/null 2>&1 || { echo "Error: Node.js 20+ and npm are required only for the optional Electron UI."; exit 2; }
	@command -v npm >/dev/null 2>&1 || { echo "Error: npm is required only for the optional Electron UI."; exit 2; }

# Evaluation-critical bootstrap: Cargo resolves every Rust dependency through
# the committed lockfile. It deliberately does not require Python or Node.
setup: check-git check-rust rust-build-release
	@$(LOAD_LOCAL_ENV) \
	if [ "$(CONFIGURE)" = "1" ]; then \
		"$(RUST_BIN)" preflight --configure; \
	else \
		"$(RUST_BIN)" preflight --noninteractive; \
	fi
	@echo "==> Setup complete. Run 'make run' to start a mission."

setup-python: check-python
	@echo "==> Setting up Python test/compatibility environment..."
	@if [ ! -x "$(VENV)/bin/python" ]; then $(PYTHON) -m venv "$(VENV)"; fi
	@$(VENV)/bin/python -c 'import pytest, setuptools' >/dev/null 2>&1 || \
		$(VENV)/bin/python -m pip install --disable-pip-version-check 'setuptools>=68' -r requirements-dev.txt
	@$(VENV)/bin/python -m pip install --disable-pip-version-check --no-build-isolation -e .

setup-desktop: check-node
	@echo "==> Installing optional Electron UI dependencies from package-lock.json..."
	@npm --prefix "$(DESKTOP_DIR)" ci

config:
	@$(LOAD_LOCAL_ENV) \
	provider="$${SECONDEGO_PROVIDER:-}"; \
	model="$${SECONDEGO_MODEL:-}"; \
	if [ -z "$$provider" ]; then case "$$model" in gemini-*) provider=gemini ;; llama-*|mixtral-*) provider=groq ;; *) provider=deepseek ;; esac; fi; \
	if [ -z "$$model" ]; then case "$$provider" in gemini) model=gemini-3.8-flash ;; groq) model=qwen/qwen3.8-27b ;; *) model=deepseek-flash ;; esac; fi; \
	case "$$provider" in groq) key_name=GROQ_API_KEY; key_value="$${GROQ_API_KEY:-}" ;; *) key_name=AI_API_KEY; key_value="$${AI_API_KEY:-}" ;; esac; \
	if [ -n "$$key_value" ]; then key_status=present; else key_status=missing; fi; \
	if [ "$${VOICE_ENABLED:-false}" = "true" ] || [ "$${VOICE_ENABLED:-false}" = "1" ]; then voice_status=enabled; else voice_status=disabled; fi; \
	if [ -n "$${GEMINI_API_KEY:-}" ]; then voice_key_status=present; else voice_key_status=missing; fi; \
	voice_model="$${GEMINI_TTS_MODEL:-gemini-3.8-flash-lite-tts}"; \
	ui_status="$${SECONDEGO_SURFACE:-terminal}"; \
	if [ "$$ui_status" = "desktop" ] || [ "$(UI)" = "on" ]; then ui_status=desktop; else ui_status=terminal; fi; \
	if [ "$${SECONDEGO_GESTURES_ENABLED:-false}" = "true" ] || [ "$${SECONDEGO_GESTURES_ENABLED:-false}" = "1" ]; then gesture_status=enabled; else gesture_status=disabled; fi; \
	printf 'SecondEgo configuration: provider=%s model=%s %s=%s surface=%s terminal=%s voice=%s voice_model=%s GEMINI_API_KEY=%s gestures=%s\n' "$$provider" "$$model" "$$key_name" "$$key_status" "$$ui_status" "$(UI_MODE)" "$$voice_status" "$$voice_model" "$$voice_key_status" "$$gesture_status"

run:
	@$(LOAD_LOCAL_ENV) \
	if [ "$(UI)" = "on" ]; then export SECONDEGO_SURFACE=desktop; fi; \
	if [ -n "$(PROFILE)" ]; then export SECONDEGO_PROFILE="$(PROFILE)"; fi; \
	if [ "$(NONINTERACTIVE)" = "1" ]; then export SECONDEGO_NONINTERACTIVE=true; fi; \
	export SECONDEGO_TERMINAL_MODE="$${SECONDEGO_TERMINAL_MODE:-$(UI_MODE)}"; \
	if [ "$(ENGINE)" = "python" ]; then \
		$(MAKE) run-python UI_MODE="$(UI_MODE)"; \
	else \
		$(MAKE) run-rust UI_MODE="$(UI_MODE)"; \
	fi

run-rust: check-git check-rust
	@$(MAKE) rust-build-release
	@SECONDEGO_TERMINAL_MODE="$${SECONDEGO_TERMINAL_MODE:-$(UI_MODE)}" "$(RUST_BIN)" launch; \
	code=$$?; \
	if [ "$$code" -eq 130 ]; then exit 0; fi; \
	exit "$$code"

# Optional presentation. The evaluator-facing default remains headless.
ui:
	$(MAKE) desktop-electron

# Single-command local judge entry point. The official evaluator may still run
# `make setup` and `make run` separately, exactly as specified by the rubric.
judge:
	$(MAKE) setup NONINTERACTIVE=1
	$(MAKE) run PROFILE=judge NONINTERACTIVE=1

run-python:
	@$(LOAD_LOCAL_ENV) \
	$(MAKE) setup-python && \
	case "$${SECONDEGO_PROVIDER:-deepseek}" in \
		groq) test -n "$${GROQ_API_KEY:-}" || { echo "Error: GROQ_API_KEY must be exported or placed in the ignored local .env file before make run-python."; exit 2; } ;; \
		*) test -n "$${AI_API_KEY:-}" || { echo "Error: AI_API_KEY must be exported or placed in the ignored local .env file before make run-python."; exit 2; } ;; \
	esac; \
	SECONDEGO_UI_MODE="$(UI_MODE)" PATH="$(CURDIR)/$(VENV)/bin:$$PATH" $(VENV)/bin/secondego-tui

help: rust-build
	"$(CURDIR)/engine-rs/target/debug/secondego-cli" --help

discover: rust-build
	@test -n "$(REPO)" || (echo "Usage: make discover REPO=/path/to/repository [LENSES=error,test,structural]"; exit 2)
	"$(CURDIR)/engine-rs/target/debug/secondego-cli" discover --repo "$(REPO)" $(if $(LENSES),--lens "$(LENSES)",)

desktop:
	@$(LOAD_LOCAL_ENV) \
	$(MAKE) setup-python && \
	PATH="$(CURDIR)/$(VENV)/bin:$$PATH" $(VENV)/bin/secondego-desktop

desktop-electron:
	@$(LOAD_LOCAL_ENV) \
	$(MAKE) setup && \
	$(MAKE) setup-desktop && \
	if [ "$(USE_MACOS_NATIVE)" = "1" ] && [ "$(shell uname -s)" = "Darwin" ]; then \
		$(MAKE) desktop-macos; \
	else \
		npm --prefix "$(DESKTOP_DIR)" run desktop; \
	fi

desktop-macos:
	@$(MAKE) check-python
	@$(MAKE) setup-desktop
	@npm --prefix "$(DESKTOP_DIR)" run build
	@mkdir -p apps/desktop/native/.build/SecondEgo.app/Contents/MacOS apps/desktop/native/.build/SecondEgo.app/Contents/Resources apps/desktop/native/.build/module-cache
	@CLANG_MODULE_CACHE_PATH="$(CURDIR)/apps/desktop/native/.build/module-cache" swiftc -target "$(shell uname -m)-apple-macosx11.0" -framework Cocoa -framework WebKit -O -o apps/desktop/native/.build/SecondEgo.app/Contents/MacOS/SecondEgo apps/desktop/native/SecondEgo.swift
	@cp apps/desktop/native/Info.plist apps/desktop/native/.build/SecondEgo.app/Contents/Info.plist
	@cp apps/desktop/native/SecondEgo.icns apps/desktop/native/.build/SecondEgo.app/Contents/Resources/SecondEgo.icns
	@$(LOAD_LOCAL_ENV) \
	port="$${SECONDEGO_GATEWAY_PORT:-$$(python3 -c 'import socket; s=socket.socket(); s.bind(("127.0.0.1", 0)); print(s.getsockname()[1]); s.close()')}"; \
	cleanup() { pid="$$(lsof -tiTCP:"$$port" -sTCP:LISTEN 2>/dev/null | head -1)"; if [ -n "$$pid" ]; then kill "$$pid" 2>/dev/null || true; fi; }; \
	on_signal() { cleanup; exit 130; }; \
	trap cleanup EXIT; trap on_signal INT TERM; \
	SECONDEGO_ROOT="$(CURDIR)" SECONDEGO_GATEWAY_PORT="$$port" apps/desktop/native/.build/SecondEgo.app/Contents/MacOS/SecondEgo; \
	code=$$?; \
	if [ "$$code" -eq 130 ]; then exit 0; fi; \
	exit "$$code"

rust-check:
	@$(MAKE) check-rust
	cargo check --manifest-path "$(RUST_MANIFEST)" --locked

rust-test:
	@$(MAKE) check-rust
	cargo test --manifest-path "$(RUST_MANIFEST)" --locked

rust-build:
	@$(MAKE) check-rust
	cargo build --manifest-path "$(RUST_MANIFEST)" --locked --quiet

rust-build-release:
	@$(MAKE) check-rust
	@echo "==> Building evaluator harness from Cargo.lock..."
	cargo build --manifest-path "$(RUST_MANIFEST)" --locked --release -p secondego-cli -p secondego-gateway --quiet

rust-run:
	@$(MAKE) check-rust
	cargo run --manifest-path "$(RUST_MANIFEST)" --locked -p secondego-cli -- --workspace "$(CURDIR)" --task "$(TASK)" $(if $(SCRIPT),--script "$(SCRIPT)",)

gc:
	PYTHONPATH="$(CURDIR)/src" $(PYTHON) -c 'from SecondEgo.lifecycle import collect_garbage; print(collect_garbage())'

test: setup-python
	$(VENV)/bin/python -m pytest -q
	$(MAKE) rust-test

clean:
	rm -rf $(VENV) .pytest_cache build dist src/secondego.egg-info
	find src tests -type d -name '__pycache__' -prune -exec rm -rf {} +
