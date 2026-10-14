# tokenwitness

**Audit the byte-identity of chat-template rendering across inference stacks.**
Given one logical chat (role/content messages) and two stacks (HuggingFace
`apply_chat_template`, llama.cpp `--jinja`, vLLM, TGI, Ollama, cooperating
OpenAI-compatible endpoints), report the exact bytes each stack feeds the
model, their SHA-256, and the first byte at which they diverge.

![status: initial scaffold](https://img.shields.io/badge/status-initial%20scaffold-lightgrey)
![language: Rust](https://img.shields.io/badge/language-Rust-blue)
![license: MIT](https://img.shields.io/badge/license-MIT-blue)

## Why

Published chat templates are shared across stacks, but the renderers that
consume them diverge in subtle ways: BOS token presence, trailing newline,
system-prompt wrapping, tool-call block spacing. The difference is invisible
until it changes refusal behavior, tool-call reliability, or benchmark
numbers.

The need was observed first in the `refusalscope` study, where a stock vs.
ablated comparison depended on byte-identical rendering between the two
servers. Nobody has published a cross-stack audit of this; tokenwitness fills
that gap.

## What is in this scaffold

- `Prompt`, `Message`, `Rendering`, `ByteDiff` types with serde round-trip.
- A `diff` function that computes the first differing byte, both lengths,
  and both SHA-256 hashes.
- A `tokenwitness diff` CLI that reads two renderings from JSON and prints
  the comparison.
- `docs/PREREGISTRATION.md` with the comparison method locked.

Not in this commit (tracked, deliberately missing):

- Capture adapters for HF (`apply_chat_template`), llama.cpp
  (`server.cpp`'s prompt-logging path), vLLM, TGI, Ollama, OpenAI-API.
- Standard fixtures: system-only, user-only, user+assistant, tool-call,
  thinking block, non-ASCII, trailing-whitespace.
- Side-by-side hex pretty-printer.
- Three-way diff over N stacks at once.

## Usage (scaffold)

```sh
cargo install --path .
cat > hf.json <<'EOF'
{"stack": "hf", "bytes": [60, 124, 117, 115, 101, 114, 124, 62, 104, 105]}
EOF
cat > llamacpp.json <<'EOF'
{"stack": "llamacpp", "bytes": [60, 124, 117, 115, 101, 114, 124, 62, 104, 105, 10]}
EOF
tokenwitness diff hf.json llamacpp.json
# verdict: same prefix but len differs (len_a != len_b)
```

## Methodology commitments

- No numbers in any report that are not produced from a `fixtures/*.json`
  input and a captured `renderings/*.json`.
- A pair of stacks that produces identical bytes is reported as *identical*
  for that fixture only. A single fixture never generalizes to a model.
- Templates (jinja files) are themselves inputs and are version-pinned per
  model release in the fixture; a change in the template between captures
  is a new row, not a correction to an old one.

See `docs/PREREGISTRATION.md`.

## License

MIT. See `LICENSE`.
