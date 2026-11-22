# shrug 🤷

A **System One / decisions-compatible** HTTP API that answers with a principled
**random draw**. No model, no inference, no network round-trip to a GPU — just a
sub-millisecond response shaped exactly like a real decision endpoint.

shrug isn't a System One *model*. It speaks the protocol but answers by sampling —
think of it as **System null**: the chance-level baseline. That makes it two useful
things at once:

- **A drop-in mock** for the OpenAI Decisions / TypeSafe System One shape — in tests,
  local dev, CI, and demos. Same request and response, instant and free, no keys to
  manage, nothing to go flaky.
- **A chance-level baseline** for evals — if a real decision model can't beat a random
  shrug, that's a result worth knowing.

It does no thinking, so locally it's **sub-millisecond** — thousands of times faster
than any model you could run on the same machine, because those run real inference and
this runs a random number generator.

## Quick start

### Docker

```bash
docker run --rm -p 8080:8080 ghcr.io/infinitybricks/shrug
```

### Cargo

```bash
cargo install shrug
shrug   # listens on 0.0.0.0:8080
```

### From source

```bash
git clone https://github.com/infinitybricks/shrug && cd shrug
cargo run --release -p shrug
```

Then make a decision:

```bash
curl -s localhost:8080/v1/systemone -H 'content-type: application/json' -d '{
  "model": "system-one",
  "state": "I was charged twice. Please refund the duplicate charge.",
  "questions": {
    "route":   { "type": "choice", "instructions": "Which team?",
                 "criteria": { "billing": "Billing", "technical_support": "Tech" } },
    "refund":  { "type": "noul",   "instructions": "Asking for a refund?" },
    "urgency": { "type": "score",  "instructions": "How urgent?",
                 "criteria": ["Routine", "Today", "Emergency"] }
  }
}'
```

Both shapes are served: `POST /v1/systemone` (TypeSafe System One) and
`POST /v1/decisions` (OpenAI Decisions), plus `GET /livez` and `GET /readyz`.

## Bias the dice (optional)

Add a `shrug.distributions` block to weight a choice by name or set `p_true` for a
predicate. Weights are normalized, so you control the null hypothesis:

```jsonc
"shrug": { "distributions": {
  "route":  { "weights": { "billing": 0.7, "technical_support": 0.3 } },
  "refund": { "p_true": 0.9 }
} }
```

## Use it as a library

The draw engine is a separate crate with no server dependencies:

```toml
[dependencies]
shrug-core = "0.1"
```

## Configuration

| Env var          | Default        | Meaning                            |
| ---------------- | -------------- | ---------------------------------- |
| `BIND_ADDR`      | `0.0.0.0:8080` | Listen address.                    |
| `MAX_BODY_BYTES` | `262144`       | Request body cap (413 above it).   |

There is no authentication and no database — shrug serves anyone who can reach it.
It's built for local development, tests, CI, and benchmarking.

## Hosted

Want a managed, authenticated, metered endpoint with an API key and a dashboard?
That's [shrug.ai](https://shrug.ai). This repository is the open-source engine behind
it — the same draws, without the SaaS.

## License

Licensed under either of [MIT](./LICENSE-MIT) or [Apache-2.0](./LICENSE-APACHE) at your
option.
