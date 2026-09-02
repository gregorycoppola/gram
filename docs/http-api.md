# HTTP API

Start the local service from the repository root:

```sh
cargo run -- serve --port 9101 --fixtures-dir fixtures \
  --proofs-dir proofs --data-dir ../gram-data
```

The service binds to `127.0.0.1` and currently permits any CORS origin. It has
no authentication and is intended only for local development.

| Method | Path | Purpose |
|---|---|---|
| GET | `/health` | Return `{ "status": "ok" }` |
| GET | `/fixtures` | List syntax fixtures |
| GET | `/fixtures/:name` | Return a fixture |
| GET | `/fixtures/:name/parse` | Parse all fixture sentences |
| POST | `/parse/one` | Parse one sentence using a named fixture |
| GET | `/proofs` | List proof fixtures |
| GET | `/proofs/:name` | Return a proof fixture |
| POST | `/proof/check` | Validate a proof document |
| GET | `/inference/fixtures` | List QBBN fixtures |
| POST | `/inference/run` | Run an inference fixture document |
| GET | `/coverage` | Summarize corpus articles and examples |
| GET | `/coverage/:name` | Return a corpus fixture |
| GET | `/coverage/:name/parse` | Parse a corpus fixture |

Nested names are URL encoded by the Gram CLI and Gloss client. Most stable
operations can be exercised with `cargo run -- api --help`.

The API has no version endpoint, compatibility negotiation, published schema,
or stability guarantee yet. Error bodies are implementation-defined. Treat
every route as experimental until these contracts are added.
