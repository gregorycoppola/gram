# Limitations

Gram is an experimental research implementation, not a broad-coverage natural
language system.

- Grammar and lexicon coverage is fixture-driven and deliberately small.
- Annotated spans are part of the current parsing workflow; arbitrary raw text
  is not expected to work without suitable fixture data.
- Proof checking validates supplied derivations but does not perform general
  proof search.
- Exact inference scales exponentially and is intended as an oracle for small
  graphs. Belief propagation can be approximate on loopy graphs.
- Public Rust APIs, CLI output, HTTP payloads, and persisted JSON have no
  compatibility guarantees yet.
- Fixture versions are advertised by the service rather than embedded in each
  document, and there is not yet a published JSON Schema.
- The HTTP server is local-only, unauthenticated, and permissive about CORS.
- Corpus coverage depends on separately obtained data whose redistribution
  status must be reviewed before release.

Results should be interpreted as demonstrations over the included fixtures,
not claims of general English-language coverage or calibrated real-world
probabilities.
