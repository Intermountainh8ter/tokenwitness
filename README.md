# tokenwitness

**Audit the byte-identity of chat-template rendering across inference stacks.**
Given one logical chat (role/content messages) and two stacks (HuggingFace
`apply_chat_template`, llama.cpp `--jinja`, vLLM, TGI, Ollama, cooperating
OpenAI-compatible endpoints), report the exact bytes each stack feeds the
model, their SHA-256, and the first byte at which they diverge.

![status: first live captures](https://img.shields.io/badge/status-first%20live%20captures-blue)
![language: Rust](https://img.shields.io/badge/language-Rust-blue)
![license: MIT](https://img.shields.io/badge/license-MIT-blue)

## Results

<!-- RESULTS:START -->
| fixture | model | stack A (version) | stack B (version) | template A = B | bytes A / B | verdict |
|---|---|---|---|---|---|---|
| qwen3-4b-instruct-2507-chat | Qwen/Qwen3-4B-Instruct-2507 | hf (transformers 5.20.1) | llamacpp (b1-091b6a7) | yes | 313 / 313 | identical |
| qwen3-4b-instruct-2507-chat | Qwen/Qwen3-4B-Instruct-2507 | hf (transformers 5.20.1) | minijinja (minijinja 2) | yes | 313 / 313 | identical |
| qwen3-4b-instruct-2507-chat | Qwen/Qwen3-4B-Instruct-2507 | llamacpp (b1-091b6a7) | minijinja (minijinja 2) | yes | 313 / 313 | identical |
| qwen3-4b-instruct-2507-nosystem | Qwen/Qwen3-4B-Instruct-2507 | hf (transformers 5.20.1) | llamacpp (b1-091b6a7) | yes | 116 / 116 | identical |
| qwen3-4b-instruct-2507-nosystem | Qwen/Qwen3-4B-Instruct-2507 | hf (transformers 5.20.1) | minijinja (minijinja 2) | yes | 116 / 116 | identical |
| qwen3-4b-instruct-2507-nosystem | Qwen/Qwen3-4B-Instruct-2507 | llamacpp (b1-091b6a7) | minijinja (minijinja 2) | yes | 116 / 116 | identical |

<!-- RESULTS:END -->

Rendered by `tokenwitness report` from `renderings/*.json` (each capture carries its bytes, SHA-256, stack version, method and template hash). Fixtures are in `fixtures/`; a new fixture or template hash is a new row, never a replacement.

## Capturing

```
tokenwitness capture --fixture fixtures/<f>.json --stack hf --python <venv>/bin/python --model-dir <hf model dir> --out renderings/<f>-hf.json
tokenwitness capture --fixture fixtures/<f>.json --stack llamacpp --url http://127.0.0.1:8092 --out renderings/<f>-llamacpp.json
tokenwitness capture --fixture fixtures/<f>.json --stack minijinja --template <file.jinja> --out renderings/<f>-minijinja.json
tokenwitness report
```

The llama.cpp capture uses `llama-server`'s `/apply-template` endpoint, which runs the same template code the server uses before tokenising a chat request, rather than a log line; the server build and its loaded template are recorded with each capture. The minijinja capture is an in-process render with the engine Rust inference stacks such as mistral.rs use, with Python-compatibility methods enabled.

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
