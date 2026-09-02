# Gram

Gram is an experimental natural-language-to-probabilistic-logic compiler and
reasoning runtime written in Rust. It combines a typed slot grammar, logical
form construction, machine-checkable proofs, and QBBN inference behind a Rust
library, command-line interface, and local HTTP service.

> **Research status:** Gram is a pre-release research artifact. Its Rust API,
> command-line output, HTTP API, and JSON fixture formats are not yet stable or
> versioned. It is suitable for experimentation, not production use.

## What it does

- tokenizes fixture sentences and compiles typed grammar rules;
- preserves alternative derivations and compares their semantics with a gold
  logical form;
- parses and type-checks logical forms;
- validates explicit formal-proof steps;
- builds QBBN factor graphs and runs exact or belief-propagation inference;
- exposes the same fixtures and operations to the companion Gloss workbench.

## Quick start

Prerequisites: Rust 1.92 or newer is known to work. A lower minimum supported
Rust version has not been established.

```sh
git clone https://github.com/gregorycoppola/gram.git
cd gram
cargo test --all-targets --all-features

# Parse ten fixture sentences and compare them with gold semantics.
cargo run -- parse --fixture fixtures/rc2_transitive.json

# Check a formal proof.
cargo run -- check --proof proofs/regression/socrates_mortal.json

# Compare exact and belief-propagation inference.
cargo run -- infer --fixture fixtures/qbbn/socrates_mortal.json
```

The parse command should report a gold match for each sentence. The proof
command should end with `Proof valid`, and the inference example should report
that all topology-appropriate checks passed.

## Run Gram with Gloss

Keep Gram and Gloss in sibling directories. Corpus coverage also expects the
optional `gram-data` repository to be a sibling by default:

```text
workspace/
├── gram/
├── gloss/
└── gram-data/       # optional except for the coverage view
```

Start the API from the Gram repository:

```sh
cargo run -- serve \
  --port 9101 \
  --fixtures-dir fixtures \
  --proofs-dir proofs \
  --data-dir ../gram-data
```

Verify it in another terminal:

```sh
cargo run -- api health
```

Then follow the Gloss README and open `http://127.0.0.1:3334/gloss/`.

## Library use

The crate currently exposes `cli`, `core`, and `server` directly. These are
implementation modules, not a promised stable facade. Downstream library users
should pin an exact revision until a supported public API is introduced.

Generate current API documentation locally with:

```sh
cargo doc --no-deps --open
```

## CLI

```text
gram parse       Parse every sentence in a fixture
gram parse-one   Parse one sentence using a fixture grammar and lexicon
gram inspect     Summarize a fixture
gram tree        Display parse trees
gram check       Validate a proof file
gram infer       Run a QBBN inference fixture
gram serve       Start the local HTTP service (default port 9101)
gram api         Call a running Gram service
```

Run `cargo run -- <command> --help` for the current flags. CLI output and exit
behavior are not yet a versioned interface.

## Development checks

```sh
cargo fmt --all -- --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test --all-targets --all-features
```

See [docs/architecture.md](docs/architecture.md),
[docs/http-api.md](docs/http-api.md), and
[docs/limitations.md](docs/limitations.md) for the current design and contract
boundaries. QBBN semantics and testing are described in
[docs/qbbn-inference.md](docs/qbbn-inference.md).

## Related repositories

- [Gloss](https://github.com/gregorycoppola/gloss) is the SolidJS inspection
  workbench for syntax, proofs, inference, and corpus coverage.
- [Gram Data](https://github.com/gregorycoppola/gram-data) contains experimental
  real-language fixtures and is required only for the coverage endpoints.

## Contributing and license

Contributor guidance and governance documents are being prepared for the first
public release. A license has not yet been selected; do not assume permission
to redistribute this repository until an explicit license file is added.
