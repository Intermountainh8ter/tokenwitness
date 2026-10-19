//! Capture adapters: one canonical way per stack to obtain the bytes that
//! stack feeds the model for a fixture (docs/PREREGISTRATION.md section 3).
//!
//! * `hf`: `transformers.AutoTokenizer.apply_chat_template(..., tokenize=False)`
//!   run in a named Python interpreter against a local model directory. The
//!   adapter writes the rendered string to a temporary file as UTF-8 bytes;
//!   nothing is normalised on the way back.
//! * `llamacpp`: `llama-server`'s `/apply-template` endpoint, which renders
//!   with the same `common_chat_templates_apply` path the server uses before
//!   tokenising a chat completion. The server's build and the template it
//!   loaded (from `/props`) are recorded with the capture.
//! * `minijinja`: an in-process render of a jinja file with the minijinja
//!   engine, the renderer used by Rust inference stacks such as mistral.rs.
//!   It is a stack in its own right, not a reference answer.
//!
//! Every capture records the stack, its version, the method, and the SHA-256
//! of the template it used, because a different template is a different row.

use crate::{Prompt, Rendering};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::path::Path;

/// A rendering plus the provenance the pre-registration asks for.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Capture {
    pub fixture: String,
    pub model: String,
    pub stack: String,
    pub stack_version: String,
    pub method: String,
    pub template_sha256: String,
    pub add_generation_prompt: bool,
    pub sha256: String,
    pub len: usize,
    pub rendering: Rendering,
}

fn sha_hex(b: &[u8]) -> String {
    format!("{:x}", Sha256::digest(b))
}

pub fn finish(fixture: &Prompt, stack: &str, stack_version: &str, method: &str, template: &[u8], bytes: Vec<u8>) -> Capture {
    let rendering = Rendering { stack: stack.to_string(), bytes };
    Capture {
        fixture: fixture.name.clone(),
        model: fixture.model.clone(),
        stack: stack.to_string(),
        stack_version: stack_version.to_string(),
        method: method.to_string(),
        template_sha256: sha_hex(template),
        add_generation_prompt: fixture.add_generation_prompt,
        sha256: rendering.sha256(),
        len: rendering.bytes.len(),
        rendering,
    }
}

/// Hugging Face transformers in a given interpreter. The model directory must hold
/// `tokenizer_config.json` with a `chat_template`.
pub fn capture_hf(fixture: &Prompt, python: &Path, model_dir: &Path) -> anyhow::Result<Capture> {
    let script = r#"
import json, sys
from transformers import AutoTokenizer
import transformers
spec = json.load(open(sys.argv[1]))
tok = AutoTokenizer.from_pretrained(sys.argv[2])
out = tok.apply_chat_template(spec["messages"], tokenize=False, add_generation_prompt=spec["add_generation_prompt"])
open(sys.argv[3], "wb").write(out.encode("utf-8"))
tpl = tok.chat_template if isinstance(tok.chat_template, str) else json.dumps(tok.chat_template)
open(sys.argv[4], "wb").write(tpl.encode("utf-8"))
print(transformers.__version__)
"#;
    let dir = std::env::temp_dir().join(format!("tokenwitness-hf-{}", std::process::id()));
    std::fs::create_dir_all(&dir)?;
    let spec = dir.join("spec.json");
    let out = dir.join("out.bin");
    let tpl = dir.join("template.jinja");
    std::fs::write(&spec, serde_json::to_vec(&serde_json::json!({"messages": fixture.messages, "add_generation_prompt": fixture.add_generation_prompt}))?)?;
    let o = std::process::Command::new(python).arg("-I").arg("-c").arg(script).arg(&spec).arg(model_dir).arg(&out).arg(&tpl).output()?;
    anyhow::ensure!(o.status.success(), "hf capture failed: {}", String::from_utf8_lossy(&o.stderr).lines().last().unwrap_or(""));
    let version = String::from_utf8_lossy(&o.stdout).trim().to_string();
    let bytes = std::fs::read(&out)?;
    let template = std::fs::read(&tpl)?;
    let _ = std::fs::remove_dir_all(&dir);
    Ok(finish(fixture, "hf", &format!("transformers {version}"), "AutoTokenizer.apply_chat_template(tokenize=False)", &template, bytes))
}

/// llama-server's `/apply-template`, with the template and build read from `/props`.
pub fn capture_llamacpp(fixture: &Prompt, url: &str) -> anyhow::Result<Capture> {
    let base = url.trim_end_matches('/');
    let props: serde_json::Value = ureq::get(&format!("{base}/props")).timeout(std::time::Duration::from_secs(20)).call()?.into_json()?;
    let template = props["chat_template"].as_str().unwrap_or("").as_bytes().to_vec();
    let build = props["build_info"].as_str().map(|s| s.to_string()).unwrap_or_else(|| "llama-server (build unknown)".into());
    let model = props["model_path"].as_str().or(props["default_generation_settings"]["model"].as_str()).unwrap_or("").to_string();
    let resp: serde_json::Value = ureq::post(&format!("{base}/apply-template"))
        .timeout(std::time::Duration::from_secs(20))
        .send_json(serde_json::json!({"messages": fixture.messages, "add_generation_prompt": fixture.add_generation_prompt}))?
        .into_json()?;
    let prompt = resp["prompt"].as_str().ok_or_else(|| anyhow::anyhow!("/apply-template returned no `prompt`"))?;
    let method = format!("POST /apply-template (server-side common_chat_templates_apply); model file {}", Path::new(&model).file_name().map(|f| f.to_string_lossy().to_string()).unwrap_or_default());
    Ok(finish(fixture, "llamacpp", &build, &method, &template, prompt.as_bytes().to_vec()))
}

/// In-process minijinja render of a jinja template file.
pub fn capture_minijinja(fixture: &Prompt, template_path: &Path) -> anyhow::Result<Capture> {
    let template = std::fs::read(template_path)?;
    let src = String::from_utf8(template.clone())?;
    let mut env = minijinja::Environment::new();
    env.set_trim_blocks(false);
    env.set_lstrip_blocks(false);
    minijinja_contrib::add_to_environment(&mut env);
    env.set_unknown_method_callback(minijinja_contrib::pycompat::unknown_method_callback);
    env.add_template("t", &src)?;
    let t = env.get_template("t")?;
    let messages: Vec<serde_json::Value> = fixture.messages.iter().map(|m| serde_json::json!({"role": m.role, "content": m.content})).collect();
    let out = t.render(minijinja::context! { messages => messages, add_generation_prompt => fixture.add_generation_prompt })?;
    let version = format!("minijinja {}", MINIJINJA_VERSION);
    Ok(finish(fixture, "minijinja", &version, "Environment::render (trim_blocks=false, lstrip_blocks=false, contrib filters)", &template, out.into_bytes()))
}

pub const MINIJINJA_VERSION: &str = "2";
