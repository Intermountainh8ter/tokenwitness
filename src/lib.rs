//! Core types and byte-identity primitives for tokenwitness.
//!
//! The question tokenwitness answers is: given one logical chat (a sequence
//! of role/content messages) and two inference stacks, do both stacks feed
//! the model the **same bytes**? Many model families (Qwen3, Llama 3,
//! Mistral) are theoretically covered by a single chat template, but in
//! practice their jinja templates differ subtly between stacks and the
//! downstream behavior changes without warning. This crate records the
//! exact bytes each stack produces and reports the diff.

pub mod capture;

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

/// A single chat message in the logical prompt.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Message {
    pub role: String,
    pub content: String,
}

/// A logical prompt the author wants rendered across stacks.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Prompt {
    pub name: String,
    pub model: String,
    pub messages: Vec<Message>,
    #[serde(default)]
    pub add_generation_prompt: bool,
}

/// What one inference stack produced for a prompt.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Rendering {
    pub stack: String,
    /// The exact bytes the stack would feed the model (prompt serialization
    /// before tokenization). Captured either by direct `apply_chat_template`
    /// call for HF-style stacks, or by reading what the server logged for
    /// llama.cpp / vLLM, or by inspecting the wire for cooperating API
    /// endpoints.
    pub bytes: Vec<u8>,
}

impl Rendering {
    pub fn sha256(&self) -> String {
        let digest = Sha256::digest(&self.bytes);
        format!("{digest:x}")
    }
}

/// A byte-level difference between two renderings.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ByteDiff {
    pub stack_a: String,
    pub stack_b: String,
    /// First byte index where the two renderings differ, or `None` if they
    /// agree over the overlap and `len(a) == len(b)`.
    pub first_diff: Option<usize>,
    pub len_a: usize,
    pub len_b: usize,
    pub sha_a: String,
    pub sha_b: String,
}

pub fn diff(a: &Rendering, b: &Rendering) -> ByteDiff {
    let first = a
        .bytes
        .iter()
        .zip(b.bytes.iter())
        .position(|(x, y)| x != y)
        .or_else(|| {
            if a.bytes.len() == b.bytes.len() {
                None
            } else {
                Some(a.bytes.len().min(b.bytes.len()))
            }
        });
    ByteDiff {
        stack_a: a.stack.clone(),
        stack_b: b.stack.clone(),
        first_diff: first,
        len_a: a.bytes.len(),
        len_b: b.bytes.len(),
        sha_a: a.sha256(),
        sha_b: b.sha256(),
    }
}

impl ByteDiff {
    pub fn identical(&self) -> bool {
        self.first_diff.is_none() && self.len_a == self.len_b
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn r(stack: &str, bytes: &[u8]) -> Rendering {
        Rendering {
            stack: stack.into(),
            bytes: bytes.to_vec(),
        }
    }

    #[test]
    fn identical_bytes_identical_diff() {
        let a = r("hf", b"<|user|>hi<|assistant|>");
        let b = r("llamacpp", b"<|user|>hi<|assistant|>");
        let d = diff(&a, &b);
        assert!(d.identical());
        assert_eq!(d.first_diff, None);
        assert_eq!(d.sha_a, d.sha_b);
    }

    #[test]
    fn trailing_newline_difference_is_detected() {
        let a = r("hf", b"<|user|>hi<|assistant|>");
        let b = r("llamacpp", b"<|user|>hi<|assistant|>\n");
        let d = diff(&a, &b);
        assert!(!d.identical());
        assert_eq!(d.first_diff, Some(23));
        assert_ne!(d.sha_a, d.sha_b);
    }

    #[test]
    fn byte_substitution_in_middle_is_detected() {
        let a = r("hf", b"<|user|>Hello<|assistant|>");
        let b = r("vllm", b"<|user|>hello<|assistant|>");
        let d = diff(&a, &b);
        assert_eq!(d.first_diff, Some(8));
    }
}
