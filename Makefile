PYTHON ?= python3
VENV ?= .venv

.PHONY: setup run test clean

setup:
	$(PYTHON) -m venv $(VENV)
	$(VENV)/bin/pip install 'setuptools>=68' 'pytest>=8.0' 'google-genai>=1.0'
	$(VENV)/bin/pip install --no-build-isolation -e .

run:
	@test -n "$$AI_API_KEY" || (echo "AI_API_KEY must be set for evaluation"; exit 2)
	$(VENV)/bin/secondego-tui

test:
	$(VENV)/bin/python -m pytest -q

clean:
	rm -rf $(VENV) .pytest_cache build dist src/secondego.egg-info
	find src tests -type d -name '__pycache__' -prune -exec rm -rf {} +
