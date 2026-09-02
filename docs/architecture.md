# Architecture

Gram currently has three entry surfaces:

```text
Rust callers ──┐
CLI commands ──┼── core ── syntax/semantics ── proof ── QBBN inference
HTTP service ──┘
```

`src/core` owns fixture loading, lexicon and grammar compilation, matching,
logical forms, proof checking, and QBBN construction and inference. `src/cli`
implements both local commands and an HTTP client. `src/server` maps the same
operations to a local Axum service used by Gloss.

## Data flow

For syntax, a JSON fixture supplies a lexicon, typed grammar rules, and one or
more tokenized sentences. Gram compiles the rules, matches the annotated spans,
constructs logical forms, retains distinct derivations, and optionally compares
the results with gold semantics using alpha equivalence.

Proof files contain premises, a target conclusion, and numbered justified
steps. The checker validates the steps deterministically; it does not search
for a proof.

Inference fixtures define propositions, probabilistic rules, evidence, and
queries. Gram grounds them into a factor graph, analyzes its topology, and can
compare belief propagation with exact enumeration on small graphs.

## Stability boundary

There is not yet a supported library facade. `src/lib.rs` exposes `cli`, `core`,
and `server` wholesale, and the CLI and server import internal modules directly.
Before a public crate release, Gram should introduce intentional syntax,
semantics, proof, inference, and fixture entry points and make all three
interfaces use them.
