//! tokenwitness CLI.
//!
//! `capture` drives one stack for one fixture and writes a capture JSON with
//! the bytes and their provenance; `diff` compares two captures; `report`
//! diffs every pair of captures per fixture under `renderings/` and renders
//! the README table. docs/PREREGISTRATION.md locks the comparison rule:
//! identical means equal length and equal SHA-256, anything else is reported
//! with the first differing byte, and nothing is normalised.

use clap::{Parser, Subcommand};
use std::path::PathBuf;
use tokenwitness::capture::Capture;
use tokenwitness::{Prompt, Rendering};

#[derive(Parser)]
#[command(name = "tokenwitness", version, about = "Chat-template byte-identity auditor")]
struct Cli {
    #[command(subcommand)]
    command: Cmd,
}

#[derive(Subcommand)]
enum Cmd {
    /// Capture what one stack feeds the model for a fixture.
    Capture {
        #[arg(long)]
        fixture: PathBuf,
        /// hf | llamacpp | minijinja
        #[arg(long)]
        stack: String,
        /// hf: Python interpreter with transformers installed.
        #[arg(long)]
        python: Option<PathBuf>,
        /// hf: local model directory holding tokenizer_config.json.
        #[arg(long)]
        model_dir: Option<PathBuf>,
        /// llamacpp: llama-server base URL.
        #[arg(long)]
        url: Option<String>,
        /// minijinja: jinja template file.
        #[arg(long)]
        template: Option<PathBuf>,
        #[arg(long)]
        out: PathBuf,
    },
    /// Diff two captures (or bare renderings) and print the byte-level result.
    Diff {
        a: PathBuf,
        b: PathBuf,
    },
    /// Diff every pair of captures per fixture under a directory and render the README block.
    Report {
        #[arg(long, default_value = "renderings")]
        dir: PathBuf,
        #[arg(long, default_value = "README.md")]
        readme: PathBuf,
        #[arg(long, default_value = "results/report.json")]
        out: PathBuf,
    },
    Version,
}

fn load_rendering(path: &std::path::Path) -> anyhow::Result<(Rendering, Option<Capture>)> {
    let text = std::fs::read_to_string(path)?;
    if let Ok(c) = serde_json::from_str::<Capture>(&text) {
        return Ok((c.rendering.clone(), Some(c)));
    }
    Ok((serde_json::from_str(&text)?, None))
}

fn print_diff(d: &tokenwitness::ByteDiff) {
    println!("stacks:    {} vs {}", d.stack_a, d.stack_b);
    println!("len(a):    {}", d.len_a);
    println!("len(b):    {}", d.len_b);
    println!("sha(a):    {}", d.sha_a);
    println!("sha(b):    {}", d.sha_b);
    match d.first_diff {
        None if d.identical() => println!("verdict:   identical"),
        None => println!("verdict:   same prefix but len differs (len_a != len_b)"),
        Some(i) => println!("verdict:   differ at byte {i}"),
    }
}

fn excerpt(bytes: &[u8], at: usize) -> String {
    let lo = at.saturating_sub(24);
    let hi = (at + 24).min(bytes.len());
    format!("{:?}", String::from_utf8_lossy(&bytes[lo..hi]))
}

fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();
    match cli.command {
        Cmd::Capture { fixture, stack, python, model_dir, url, template, out } => {
            let p: Prompt = serde_json::from_str(&std::fs::read_to_string(&fixture)?)?;
            let c = match stack.as_str() {
                "hf" => tokenwitness::capture::capture_hf(&p, &python.ok_or_else(|| anyhow::anyhow!("--python is required for hf"))?, &model_dir.ok_or_else(|| anyhow::anyhow!("--model-dir is required for hf"))?)?,
                "llamacpp" => tokenwitness::capture::capture_llamacpp(&p, &url.ok_or_else(|| anyhow::anyhow!("--url is required for llamacpp"))?)?,
                "minijinja" => tokenwitness::capture::capture_minijinja(&p, &template.ok_or_else(|| anyhow::anyhow!("--template is required for minijinja"))?)?,
                other => anyhow::bail!("unknown stack {other} (hf | llamacpp | minijinja)"),
            };
            if let Some(d) = out.parent() { std::fs::create_dir_all(d)?; }
            std::fs::write(&out, serde_json::to_string_pretty(&c)?)?;
            println!("{} {} {} bytes sha256 {} template {}", c.fixture, c.stack, c.len, &c.sha256[..16], &c.template_sha256[..16]);
        }
        Cmd::Diff { a, b } => {
            let (ra, _) = load_rendering(&a)?;
            let (rb, _) = load_rendering(&b)?;
            print_diff(&tokenwitness::diff(&ra, &rb));
        }
        Cmd::Report { dir, readme, out } => {
            let mut caps: Vec<Capture> = Vec::new();
            let mut files: Vec<PathBuf> = std::fs::read_dir(&dir)?.filter_map(|e| e.ok()).map(|e| e.path()).filter(|p| p.extension().map_or(false, |e| e == "json")).collect();
            files.sort();
            for f in &files {
                if let (_, Some(c)) = load_rendering(f)? { caps.push(c); }
            }
            let mut fixtures: Vec<String> = caps.iter().map(|c| c.fixture.clone()).collect();
            fixtures.sort();
            fixtures.dedup();
            let mut rows = Vec::new();
            let mut t = String::from("| fixture | model | stack A (version) | stack B (version) | template A = B | bytes A / B | verdict |\n|---|---|---|---|---|---|---|\n");
            for fx in &fixtures {
                let cs: Vec<&Capture> = caps.iter().filter(|c| &c.fixture == fx).collect();
                for i in 0..cs.len() {
                    for j in i + 1..cs.len() {
                        let (a, b) = (cs[i], cs[j]);
                        let d = tokenwitness::diff(&a.rendering, &b.rendering);
                        let verdict = match d.first_diff {
                            None if d.identical() => "identical".to_string(),
                            None => "same prefix, lengths differ".to_string(),
                            Some(k) => format!("differ at byte {k}: A {} vs B {}", excerpt(&a.rendering.bytes, k), excerpt(&b.rendering.bytes, k)),
                        };
                        let same_tpl = if a.template_sha256 == b.template_sha256 { "yes" } else { "no" };
                        t.push_str(&format!("| {} | {} | {} ({}) | {} ({}) | {} | {} / {} | {} |\n", fx, a.model, a.stack, a.stack_version, b.stack, b.stack_version, same_tpl, d.len_a, d.len_b, verdict.replace('|', "\\|")));
                        rows.push(serde_json::json!({"fixture": fx, "model": a.model, "stack_a": a.stack, "version_a": a.stack_version, "stack_b": b.stack, "version_b": b.stack_version, "template_a": a.template_sha256, "template_b": b.template_sha256, "sha_a": d.sha_a, "sha_b": d.sha_b, "len_a": d.len_a, "len_b": d.len_b, "first_diff": d.first_diff, "identical": d.identical()}));
                    }
                }
            }
            if rows.is_empty() { t.push_str("| (no captures yet) | | | | | | |\n"); }
            if let Some(d) = out.parent() { std::fs::create_dir_all(d)?; }
            std::fs::write(&out, serde_json::to_string_pretty(&serde_json::json!({"tool": format!("tokenwitness {}", env!("CARGO_PKG_VERSION")), "pairs": rows}))?)?;
            let readme_text = std::fs::read_to_string(&readme)?;
            let (s, e) = ("<!-- RESULTS:START -->", "<!-- RESULTS:END -->");
            let (a, b) = (readme_text.find(s).ok_or_else(|| anyhow::anyhow!("README lacks {s}"))?, readme_text.find(e).ok_or_else(|| anyhow::anyhow!("README lacks {e}"))?);
            std::fs::write(&readme, format!("{}{}\n{}\n{}", &readme_text[..a], s, t, &readme_text[b..]))?;
            print!("{t}");
        }
        Cmd::Version => println!("tokenwitness {}", env!("CARGO_PKG_VERSION")),
    }
    Ok(())
}
