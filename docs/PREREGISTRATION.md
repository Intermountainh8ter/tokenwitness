# tokenwitness pre-registration

Locked before any real capture ships. Append-only after the first commit
under `fixtures/` or `renderings/`.

## 1. The claim this repo makes and does not make

tokenwitness compares, for one fixture, the **bytes a specific stack would
feed the model** before tokenization. It reports whether two stacks produce
the same bytes.

It does **not** claim:
- that the tokens produced from those bytes are identical (two different
  byte strings can tokenize to the same token sequence; two identical byte
  strings can tokenize differently if the tokenizer differs);
- that behavioral outputs are the same (even identical bytes can diverge
  on sampling or decoding settings);
- that a chat-template render generalizes beyond the exact
  fixture (model, template, message set).

## 2. Fixtures

The canonical fixture set lives in `fixtures/` and is append-only. Each
fixture is a JSON file carrying:

- `name`: short slug.
- `model`: HF model id (e.g. `Qwen/Qwen3-8B`).
- `template_hash`: SHA-256 of the jinja file used.
- `messages`: list of `{role, content}` entries.
- `add_generation_prompt`: boolean.

A new fixture or a new `template_hash` is a new row in every report; it
never replaces an earlier row.

## 3. Capture rules per stack

Each stack has one canonical way to produce the bytes:

- **hf** — `tokenizers.apply_chat_template(messages, tokenize=False,
  add_generation_prompt=…)`.
- **llamacpp** — `llama-server`'s own prompt-serialization log line (the
  one printed just before tokenization).
- **vllm** — vLLM's `enable_prompt_tokens_details` or the
  `prompt_token_ids`-paired `prompt` field, decoded by its own tokenizer.
- **tgi** — the `info` endpoint plus the prompt-log line.
- **ollama** — the chat endpoint's own prompt echo.
- **openai-compat / anthropic / mistral** — endpoints that cooperate by
  echoing the serialized prompt in a response field, when they do; those
  that do not are marked `?` and not compared.

Each capture is produced by one of the `capture adapters` in this repo
(TODO). Hand-written renderings are allowed in the initial scaffold
but every result committed afterward must be adapter-produced and
reproducible.

## 4. Decision rule

For a fixture `f` and stacks `A, B`:

- **identical** iff `len(bytes_A) == len(bytes_B)` and `sha256(bytes_A) ==
  sha256(bytes_B)`.
- **first-diff** otherwise: the earliest byte index where they differ, or
  the length of the shorter capture if one is a prefix of the other.

The identical/first-diff flag is the only verdict. No fuzziness, no
normalization. Normalizing bytes before compare (lower-case, newline
collapse) would hide the exact failure mode we are trying to surface.

## 5. Threats to validity

- **Capture adapter correctness.** The capture adapters are the
  specification of "what this stack feeds the model." A bug there is the
  bug and must be reported as a known issue in `docs/THREATS_TO_VALIDITY.md`,
  not corrected silently.
- **Stacks that do not expose their prompt bytes.** Some endpoints do not;
  for those we report `?` and never claim identity or difference.
- **Non-determinism in some stacks** (e.g. feature flags, environment
  variables that change the renderer). Each capture records the stack
  version and relevant flags; a different flag is a different stack row.
