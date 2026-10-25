# Batch retrieval on rust-vulns

Reproduce the retrieval-evaluation part of [#1801](https://github.com/xerj-org/xerj/issues/1801).
The CLI and MCP adapter shipped in [#2333](https://github.com/xerj-org/xerj/pull/2333).
This harness measures that implementation; it changes no engine or MCP behavior.

## Run

From the repository root, with Python 3.9+ and GitHub CLI installed:

```sh
set -eu
(cd engine && cargo +1.97.1 build --locked --offline --release -j 32 -p xerj-server)
XERJ_BENCH_ROOT=$(pwd)
XERJ_BENCH_TMP=$(mktemp -d)
python3 -m venv "$XERJ_BENCH_TMP/venv"
"$XERJ_BENCH_TMP/venv/bin/pip" install -r tools/benchmarks/batch-retrieval/requirements.txt
gh release download pack-rust-vulns-2026-10-24 --repo xerj-org/xerj \
  --pattern rust-vulns-pack.zip --dir "$XERJ_BENCH_TMP"
"$XERJ_BENCH_TMP/venv/bin/python" tools/benchmarks/batch-retrieval/run.py \
  --pack "$XERJ_BENCH_TMP/rust-vulns-pack.zip" \
  --binary "$XERJ_BENCH_ROOT/engine/target/release/xerj" \
  --engine-commit "$(git rev-parse HEAD)" \
  --output "$XERJ_BENCH_TMP/run" --repeats 5
```

The offline Cargo build needs cached dependencies; omit `--offline` on a first
build. Cargo runs from `engine/` to load its target-specific configuration.
Use a release binary built from the
declared engine commit; the harness records its SHA-256 and version but cannot
prove its source ancestry. Indexing and timing use that same binary.

The archive is pinned by its GitHub release asset SHA-256:
`54f0419e6d944635eeaf9199923064278c36fd578213db60ca77cc85d2a69e3d`.
It contains 2,051 identity-resolved records from RustSec and OSV (CC0-1.0).
The harness hashes the archive before opening it, checks manifest shard hashes
and counts, and reads ZIP members directly without extraction. It uses no Hub
pack installation, signature verification, or executable code from the archive.
The digest pins the bytes under evaluation; it is not a signature claim.

All index data is written under a fresh, private temporary directory on random
loopback ports. No existing node, index, corpus configuration or admin key is
used. The complete records are ingested with explicit keyword `id`/`aliases`
and text `title`/`body` mappings, `dynamic: false`, plus synthetic source
locators. This tests the adapter on the real pack, not autoindex schema inference
or Hub routing. Setup, bulk status, post-refresh count and final count are
checked. Children shut down normally; restart checks that the same data survives.
The temporary index is removed on exit. A failed run retains logs in the output
directory. Choose a new output path after a failed or interrupted run: existing
output directories are refused, including successful results.

The corpus is public advisory text. Only the operator can write the fresh
scratch and output directories in the intended usage. Output includes advisory
IDs/citations, public pack metadata, hardware metadata, counts and digests;
it excludes credentials and raw local source paths. Benchmark children remove
inherited proxy and `XERJ_*` configuration and disable automatic feedback so
the test only talks to its private node. No model, reranker or embedding
provider is called. The first tokenizer setup may download its fixed table.

## Workload and scoring

`cases.json` fixes the input order and labels before the run:

| Kind | Items | Success criterion |
|---|---:|---|
| Canonical ID | 15 | Exactly the labelled canonical advisory |
| CVE/GHSA alias | 15 | Exactly the labelled canonical advisory, through the explicit `aliases` field |
| Near-miss ID | 10 | `abstain: true`, `reason: no_hits` |
| Text | 8 | At least one labelled advisory in the first three hits |
| Duplicate | 1 | Same input/key as the first item; separate positional result |
| Invalid item | 1 | Independent query-validation error; never an abstention |

The eight text queries are hand-authored, title-informed known-item lookups.
Their `label_basis` is the advisory title, not a model-generated judgment.
They do not form an exhaustive relevance set: the text metric is labelled
hit@3, not answer accuracy, nDCG, full recall or calibrated confidence.
Canonical and alias targets are validated against the **full** pack before
starting the node. Canonical selection includes the eight text targets and a
SHA-256-ordered sample; aliases alternate CVE/GHSA from that ordering. Missing
IDs append `0` to a real ID and are checked absent. There are 49 valid retrieval
items and one expected item error per trial. Report error isolation separately
from retrieval accuracy; the valid-item abstention denominator is 49.

Every returned hit must cite its actual stored index and canonical ID. Exact
lookups reject extra wrong IDs. Errors, timeouts and transport failures are
never counted as successful misses. Quality is scored independently in every
trial. Ranked evidence, citations and outcomes must also agree across both
arms, both transports, warmups and repeats. A quality failure exits 1 and writes
the failed scores; malformed output, failed setup or parity failure exits 1
without publishing a successful report. Do not publish speed or cost gains
from a failed run.

## What is timed and counted

For each transport the two arms are:

- **Batch:** one `xerj_batch_search` call containing all 50 items.
- **Single:** 50 sequential one-item calls to the **same adapter**. This holds
  query semantics, mapping lookup, response schema and citation policy fixed.
  It is not a comparison with `xerj_search`, `xerj code`, grep or an LLM.

Both request `size: 3`, no `max_tokens`. All requests run at concurrency one.
One full warmup per arm is discarded. Five measured repeats alternate arm
order. CLI runs before MCP on the same refreshed, unchanged index. Caches stay
warm; no cache-flush, cold-query or read-under-write claim is made.

CLI timing includes spawning a process per call. MCP uses one initialized,
persistent stdio session; initialization and process startup are excluded.
Task latency covers the complete group of calls and JSON handling, with
scoring, hashing and BPE counting outside the timed loop. Report median,
minimum, maximum and every per-trial/per-call wall time. These are local warm
end-to-end measurements, not engine-only latency.

Tokens use `tiktoken==0.12.0`, `cl100k_base`, on the exact UTF-8 JSON lines sent
and received. CLI counts stdin/stdout; MCP counts `tools/call` request/response
envelopes including the tool name and escaped content. Each line is tokenized
separately and counts are summed, not tokenized after concatenation. Byte counts
and SHA-256 digests accompany each call. MCP initialization, tool schemas,
system prompts, conversation history and reasoning are excluded. These are
payload BPE tokens, not billed model usage or complete agent-task token cost.
They are unrelated to the adapter's UAX #29 `max_tokens` word budget.

## Harness checks

```sh
python3 tools/benchmarks/batch-retrieval/test_run.py
```

The stdlib tests cover wrong/extra IDs, error-versus-miss scoring, citation
identity, labelled hit@3, positional correspondence, parity, archive pinning,
duplicate preservation and per-call wire accounting. CI runs these checks
without downloading the corpus, tokenizer or a model. Real-node evaluation is
the explicit command above, not a network-dependent merge gate.

Generated answers, automatic alias-field discovery, confidence calibration and
escalation remain separate work under #1801 and its related issues. This
evaluation alone does not close #1801.
