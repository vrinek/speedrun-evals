//! Starts a runtime's server for one model and talks to it over the OpenAI API.
//! llama.cpp's binary comes from the Nix flake, which is what pins its version.

use crate::runner::{Completer, Completion};
use serde_json::{Value, json};
use std::{
    net::TcpListener,
    path::Path,
    process::{Child, Command, Stdio},
    thread::sleep,
    time::{Duration, Instant},
};

/// The program that runs the model.
#[derive(Clone, Copy, Debug, PartialEq, clap::ValueEnum)]
pub enum Engine {
    #[value(name = "llama.cpp")]
    LlamaCpp,
    /// ponytail: gufo is not in the flake, so it must be on PATH and its version is
    /// whatever was built there. Package it in the flake once it builds unpatched.
    Gufo,
}

impl Engine {
    /// The name recorded in results.
    pub fn name(self) -> &'static str {
        match self {
            Engine::LlamaCpp => "llama.cpp",
            Engine::Gufo => "gufo",
        }
    }

    fn binary(self) -> &'static str {
        match self {
            Engine::LlamaCpp => "llama-server",
            Engine::Gufo => "gufo",
        }
    }

    /// Every layer on the GPU, one request at a time, fixed context so runs compare.
    fn args(self, model: &Path, port: u16) -> Vec<String> {
        let (model, port) = (model.display().to_string(), port.to_string());
        let args: Vec<&str> = match self {
            Engine::LlamaCpp => vec![
                "--model",
                &model,
                "--host",
                "127.0.0.1",
                "--port",
                &port,
                "--gpu-layers",
                "999",
                "--ctx-size",
                "8192",
                "--parallel",
                "1",
            ],
            Engine::Gufo => vec![
                "serve",
                "--host",
                "127.0.0.1",
                "--port",
                &port,
                "--sessions",
                "1",
                "llm",
                "-m",
                &model,
                "--served-model-name",
                MODEL_NAME,
                "-c",
                "8192",
            ],
        };
        args.into_iter().map(String::from).collect()
    }

    /// A path that answers 200 once the model is loaded.
    fn ready_path(self) -> &'static str {
        match self {
            Engine::LlamaCpp => "/health",
            Engine::Gufo => "/v1/models",
        }
    }

    fn parse_version(self, output: &str) -> Option<String> {
        let prefix = match self {
            Engine::LlamaCpp => "version:",
            Engine::Gufo => "gufo version",
        };
        output
            .lines()
            .find_map(|line| line.strip_prefix(prefix))
            .map(|v| v.trim().to_string())
    }

    /// The runtime version recorded in results, e.g. `6123 (abc1234)`.
    pub fn version(self) -> Result<String, String> {
        let binary = self.binary();
        let out = Command::new(binary)
            .arg("--version")
            .output()
            .map_err(|e| format!("could not run {binary}: {e}"))?;
        // llama-server prints its version to stderr.
        let text = format!(
            "{}{}",
            String::from_utf8_lossy(&out.stdout),
            String::from_utf8_lossy(&out.stderr)
        );
        self.parse_version(&text)
            .ok_or(format!("no version in `{binary} --version` output"))
    }
}

/// llama.cpp lists the GPUs and their free memory whichever engine runs the model.
const SERVER: &str = "llama-server";
const LOAD_TIMEOUT: Duration = Duration::from_secs(600);
// ponytail: one generous deadline per task; the suite's max_tokens is the real limit.
const TASK_TIMEOUT: Duration = Duration::from_secs(3600);

pub struct Server {
    child: Child,
    url: String,
    agent: ureq::Agent,
}

impl Server {
    /// Starts the server and waits until the model is loaded.
    pub fn start(engine: Engine, model: &Path) -> Result<Server, String> {
        let binary = engine.binary();
        // Ask the OS for a free port. Another process could take it before the
        // server binds; the health check below then fails loudly.
        let port = TcpListener::bind("127.0.0.1:0")
            .and_then(|l| l.local_addr())
            .map_err(|e| format!("no free port: {e}"))?
            .port();
        let child = Command::new(binary)
            .args(engine.args(model, port))
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .map_err(|e| format!("could not start {binary} (run the CLI through `nix run`): {e}"))?;
        let agent = ureq::Agent::config_builder()
            .timeout_global(Some(TASK_TIMEOUT))
            .build()
            .into();
        let mut server = Server {
            child,
            url: format!("http://127.0.0.1:{port}"),
            agent,
        };

        let deadline = Instant::now() + LOAD_TIMEOUT;
        loop {
            if let Some(status) = server.child.try_wait().map_err(|e| e.to_string())? {
                return Err(format!("{binary} exited while loading the model ({status})"));
            }
            if server
                .agent
                .get(format!("{}{}", server.url, engine.ready_path()))
                .call()
                .is_ok()
            {
                return Ok(server);
            }
            if Instant::now() > deadline {
                return Err(format!("{binary} did not load the model within {LOAD_TIMEOUT:?}"));
            }
            sleep(Duration::from_millis(250));
        }
    }
}

impl Drop for Server {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

impl Completer for Server {
    fn complete(&mut self, prompt: &str, max_tokens: u32) -> Result<Completion, String> {
        let request = chat_request(prompt, max_tokens);
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

/// The name the model is served and requested under.
const MODEL_NAME: &str = "speedrun";

fn chat_request(prompt: &str, max_tokens: u32) -> Value {
    json!({
        "model": MODEL_NAME,
        "messages": [{"role": "user", "content": prompt}],
        "max_tokens": max_tokens,
        "temperature": 0,
        "seed": 0,
    })
}

fn parse_completion(response: &Value) -> Result<Completion, String> {
    let text = response["choices"][0]["message"]["content"].as_str();
    let output_tokens = response["usage"]["completion_tokens"].as_u64();
    match (text, output_tokens) {
        (Some(text), Some(output_tokens)) => Ok(Completion {
            text: text.to_string(),
            output_tokens,
        }),
        _ => Err(format!("unexpected response from the runtime: {response}")),
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
    fn requests_name_the_model_and_pin_sampling() {
        // gufo rejects a request without `model`; llama.cpp ignores the name.
        let request = chat_request("Hi", 7);
        assert_eq!(request["model"], MODEL_NAME);
        assert_eq!(request["messages"][0]["content"], "Hi");
        assert_eq!(
            (&request["max_tokens"], &request["temperature"], &request["seed"]),
            (&json!(7), &json!(0), &json!(0))
        );
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
    fn each_engine_gets_its_own_command_line() {
        let model = Path::new("/m/x.gguf");
        let llama: Vec<_> = Engine::LlamaCpp.args(model, 4000);
        assert_eq!(Engine::LlamaCpp.binary(), "llama-server");
        assert!(
            llama
                .join(" ")
                .contains("--model /m/x.gguf --host 127.0.0.1 --port 4000 --gpu-layers 999 --ctx-size 8192")
        );
        assert_eq!(Engine::Gufo.binary(), "gufo");
        assert_eq!(
            Engine::Gufo.args(model, 4000).join(" "),
            "serve --host 127.0.0.1 --port 4000 --sessions 1 llm -m /m/x.gguf --served-model-name speedrun -c 8192"
        );
    }

    #[test]
    fn finds_gufo_version() {
        assert_eq!(
            Engine::Gufo
                .parse_version("gufo version development (ef79fb2-dirty)\n")
                .as_deref(),
            Some("development (ef79fb2-dirty)")
        );
        assert_eq!(
            Engine::LlamaCpp.parse_version("version: 6123 (abc1234)\n").as_deref(),
            Some("6123 (abc1234)")
        );
    }

    #[test]
    fn finds_the_version_line() {
        let output = "ggml_vulkan: Found 1 Vulkan devices\nversion: 6123 (abc1234)\nbuilt with gcc for x86_64-linux\n";
        assert_eq!(
            Engine::LlamaCpp.parse_version(output).as_deref(),
            Some("6123 (abc1234)")
        );
        assert_eq!(Engine::LlamaCpp.parse_version("nothing here"), None);
    }
}
