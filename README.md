# gram

Typed slot grammar parser for natural language. Rust port of the spiral parser
from Paper 2 (Statistical Parsing for Logical Information Retrieval).

Reads a JSON fixture (lexicon + grammar rules + sentences) and emits parsed
logical forms.

## Usage

    cargo run --bin gram -- parse --fixture fixtures/socrates.json
    cargo run --bin gram -- parse-one --fixture fixtures/socrates.json --sentence "Socrates is a man."
    cargo run --bin gram -- parse --fixture fixtures/socrates.json --json

## QBBN inference

The QBBN subsystem includes:

- exact enumeration for small-graph semantic verification;
- edge-specific sum-product belief propagation;
- topology analysis distinguishing trees, forests, and loopy factor graphs;
- fixture output comparing BP with exact inference.

The model semantics and testing contract are documented in
`docs/qbbn-inference.md`.

## Status

v0 — pure CLI, no DB coupling. Goal is to iterate on grammar coverage using
real council sentences before wiring up persistence.