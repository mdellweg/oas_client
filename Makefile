PYTHON_VERSION = $(shell uv run --isolated python -c 'import sys; print("{}.{}".format(sys.version_info.major, sys.version_info.minor))')
RUBY_VERSION := $(shell ruby -e "puts RUBY_VERSION")
RUBY_PLATFORM := $(shell ruby -e "puts RUBY_PLATFORM")

info:
	@echo python: $(PYTHON_VERSION)
	@echo ruby: $(RUBY_VERSION)

.PHONY: format format-rust format-python format-ruby
format: format-rust format-python format-ruby

format-rust:
	cargo fmt

format-python:
	uv run ruff format
	uv run ruff check --select I --fix-only

format-ruby:
	rubocop --fix-layout

.PHONY: autofix autofix-rust autofix-python autofix-ruby
autofix: autofix-rust autofix-python autofix-ruby

autofix-rust:
	cargo fix
	cargo clippy --fix

autofix-python:
	uv run ruff check --fix
	uv run ty check --output-format concise --fix

autofix-ruby:
	rubocop --autocorrect

.PHONY: lint lint-rust lint-python lint-ruby
lint: lint-rust lint-python lint-ruby

lint-rust:
	cargo fmt --check
	cargo check
	cargo clippy

lint-python:
	uv run ruff format --check
	uv run ruff check --output-format concise
	uv run ty check --output-format concise

lint-ruby:
	rubocop

.PHONY: test test-rust test-python test-ruby
test: test-rust test-python test-ruby

test-rust:
	cargo test

test-python:
	uv run pytest -v

test-ruby: tmp/$(RUBY_PLATFORM)/rb_oas/$(RUBY_VERSION)/rb_oas.so
	rspec

tmp/$(RUBY_PLATFORM)/rb_oas/$(RUBY_VERSION)/rb_oas.so: crates/rb_oas/src/lib.rs
	rake compile
