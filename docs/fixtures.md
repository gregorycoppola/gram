# Fixture formats

Gram v0.1 recognizes three JSON fixture families. Their existing shapes are
designated version 1. The files do not embed a version field; the running
service advertises the supported versions at `GET /version`.

This choice keeps all existing Gram and Gram Data fixtures valid. A future
breaking representation must use a new advertised format version and provide a
migration path. Additive optional fields may be introduced within version 1.

## Syntax fixtures — version 1

A syntax fixture contains:

- `lexicon.predicates`: predicate names, typed roles, surface forms, and an
  optional gloss;
- `lexicon.entities`: entity names, types, surface forms, and an optional gloss;
- `grammar`: named rules with a slot pattern, semantic constructor, and kind;
- `sentences`: token arrays, labeled half-open spans, and an optional gold
  logical form.

Every sentence must be an object. Legacy plain-string sentences are rejected.
Span indices address `tokens[start..end]`; a valid hinted sentence has exactly
one full-sentence root and no crossing, duplicate, empty, or out-of-range spans.

See `fixtures/rc2_transitive.json` for a complete example. Corpus fixtures in
Gram Data use the same format.

## Proof fixtures — version 1

A proof fixture contains a `title`, a list of logical-form `premises`, the
target `conclusion`, and ordered `proof` steps. Step identifiers are sequential
and references in `from` point to earlier steps. Individual justifications may
also require a substitution or other rule-specific data.

See `proofs/regression/socrates_mortal.json` for a complete example.

## Inference fixtures — version 1

An inference fixture contains a `title`, typed `entities`, weighted logical
`rules`, probabilistic `evidence`, and `queries`. Query expectations and
tolerances are test assertions, not additional evidence.

See `fixtures/qbbn/socrates_mortal.json` for a complete example and
`docs/qbbn-inference.md` for the probabilistic semantics.

## Compatibility policy

- Readers supporting version 1 must ignore unknown additive fields.
- Required-field removal, field reinterpretation, or representation changes
  require a new integer format version.
- Gram 0.1 reports HTTP API `0.1` and fixture versions `1`.
- These contracts remain experimental and may receive a documented migration
  before 1.0; silent reinterpretation of existing version-1 data is prohibited.
