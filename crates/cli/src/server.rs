//! Starts llama.cpp's `llama-server` for one model and talks to it.
//! The binary comes from the Nix flake, which is what pins the runtime version.

use crate::runner::{Completer, Completion};
use serde_json::{Value, json};
use std::{
    net::TcpListener,
    path::Path,
    process::{Child, Command, Stdio},
    thread::sleep,
    time::{Duration, Instant},
};

const SERVER: &str = "llama-server";
const LOAD_TIMEOUT: Duration = Duration::from_secs(600);
// ponytail: one generous deadline per task; the suite's max_tokens is the real limit.
const TASK_TIMEOUT: Duration = Duration::from_secs(3600);

pub struct LlamaServer {
    child: Child,
    url: String,
    agent: ureq::Agent,
}

impl LlamaServer {
    /// Starts the server and waits until the model is loaded.
    pub fn start(model: &Path) -> Result<LlamaServer, String> {
        // Ask the OS for a free port. Another process could take it before the
        // server binds; the health check below then fails loudly.
        let port = TcpListener::bind("127.0.0.1:0")
            .and_then(|l| l.local_addr())
            .map_err(|e| format!("no free port: {e}"))?
            .port();
        let child = Command::new(SERVER)
            .arg("--model")
            .arg(model)
            .args(["--host", "127.0.0.1", "--port", &port.to_string()])
            // Offload every layer to the GPU when there is one; fixed context so runs compare.
            .args(["--gpu-layers", "999", "--ctx-size", "8192", "--parallel", "1"])
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .map_err(|e| format!("could not start {SERVER} (run the CLI through `nix run`): {e}"))?;
        let agent = ureq::Agent::config_builder()
            .timeout_global(Some(TASK_TIMEOUT))
            .build()
            .into();
        let mut server = LlamaServer {
            child,
            url: format!("http://127.0.0.1:{port}"),
            agent,
        };

        let deadline = Instant::now() + LOAD_TIMEOUT;
        loop {
            if let Some(status) = server.child.try_wait().map_err(|e| e.to_string())? {
                return Err(format!("{SERVER} exited while loading the model ({status})"));
            }
            if server.agent.get(format!("{}/health", server.url)).call().is_ok() {
                return Ok(server);
            }
            if Instant::now() > deadline {
                return Err(format!("{SERVER} did not load the model within {LOAD_TIMEOUT:?}"));
            }
            sleep(Duration::from_millis(250));
        }
    }
}

impl Drop for LlamaServer {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

impl Completer for LlamaServer {
    fn complete(&mut self, prompt: &str, max_tokens: u32) -> Result<Completion, String> {
        let request = json!({
            "messages": [{"role": "user", "content": prompt}],
            "max_tokens": max_tokens,
            "temperature": 0,
            "seed": 0,
        });
        let response: Value = self
            .agent
            .post(format!("{}/v1/chat/completions", self.url))
            .send_json(&request)
            .map_err(|e| e.to_string())?
            .body_mut()
            .read_json()
            .map_err(|e| e.to_string())?;
        parse_completion(&response)
    }
}

fn parse_completion(response: &Value) -> Result<Completion, String> {
    let text = response["choices"][0]["message"]["content"].as_str();
    let output_tokens = response["usage"]["completion_tokens"].as_u64();
    match (text, output_tokens) {
        (Some(text), Some(output_tokens)) => Ok(Completion {
            text: text.to_string(),
            output_tokens,
        }),
        _ => Err(format!("unexpected response from {SERVER}: {response}")),
    }
}

/// Raw `--list-devices` output: the GPUs the runtime can use.
pub fn list_devices() -> Result<String, String> {
    let out = Command::new(SERVER)
        .arg("--list-devices")
        .output()
        .map_err(|e| format!("could not run {SERVER} (run the CLI through `nix run`): {e}"))?;
    Ok(String::from_utf8_lossy(&out.stdout).into_owned())
}

/// The runtime version recorded in results, e.g. `6123 (abc1234)`.
pub fn version() -> Result<String, String> {
    let out = Command::new(SERVER)
        .arg("--version")
        .output()
        .map_err(|e| format!("could not run {SERVER}: {e}"))?;
    // llama-server prints its version to stderr.
    let text = format!(
        "{}{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
    parse_version(&text).ok_or(format!("no version in `{SERVER} --version` output"))
}

fn parse_version(output: &str) -> Option<String> {
    output
        .lines()
        .find_map(|line| line.strip_prefix("version:"))
        .map(|v| v.trim().to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_text_and_token_count_from_a_chat_completion() {
        let response = json!({
            "choices": [{"message": {"role": "assistant", "content": "ANSWER: 4"}, "finish_reason": "stop"}],
            "usage": {"prompt_tokens": 20, "completion_tokens": 5},
        });
        let completion = parse_completion(&response).unwrap();
        assert_eq!((completion.text.as_str(), completion.output_tokens), ("ANSWER: 4", 5));
    }

    #[test]
    fn an_error_body_is_an_error() {
        assert!(
            parse_completion(&json!({"error": {"message": "context full"}}))
                .unwrap_err()
                .contains("context full")
        );
    }

    #[test]
    fn finds_the_version_line() {
        let output = "ggml_vulkan: Found 1 Vulkan devices\nversion: 6123 (abc1234)\nbuilt with gcc for x86_64-linux\n";
        assert_eq!(parse_version(output).as_deref(), Some("6123 (abc1234)"));
        assert_eq!(parse_version("nothing here"), None);
    }
}
