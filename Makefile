PYTHON ?= python3
VENV ?= .venv
UI_MODE ?= headless

.PHONY: setup run desktop rust-check rust-test rust-build rust-run test clean

setup:
	$(PYTHON) -m venv $(VENV)
	$(VENV)/bin/pip install 'setuptools>=68' 'pytest>=8.0' 'google-genai>=1.0'
	$(VENV)/bin/pip install --no-build-isolation -e .

run:
	@test -n "$$AI_API_KEY" || (echo "AI_API_KEY must be set for evaluation"; exit 2)
	SECONDEGO_UI_MODE="$(UI_MODE)" PATH="$(CURDIR)/$(VENV)/bin:$$PATH" $(VENV)/bin/secondego-tui

desktop:
	PATH="$(CURDIR)/$(VENV)/bin:$$PATH" $(VENV)/bin/secondego-desktop

rust-check:
	cargo check --manifest-path engine-rs/Cargo.toml

rust-test:
	cargo test --manifest-path engine-rs/Cargo.toml

rust-build:
	cargo build --manifest-path engine-rs/Cargo.toml

rust-run:
	cargo run --manifest-path engine-rs/Cargo.toml -p secondego-cli -- --workspace "$(CURDIR)" --task "$(TASK)" $(if $(SCRIPT),--script "$(SCRIPT)",)

test:
	$(VENV)/bin/python -m pytest -q

clean:
	rm -rf $(VENV) .pytest_cache build dist src/secondego.egg-info
	find src tests -type d -name '__pycache__' -prune -exec rm -rf {} +
