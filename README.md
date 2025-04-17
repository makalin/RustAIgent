# RustAIgent

**RustAIgent** is a command‑line coding assistant written in Rust. It leverages large language models (LLMs) to provide interactive code assistance, file and system operations, and more. By exposing rich function‑calling tools, RustAIgent enables the LLM to:

- Inspect and modify local files
- Run shell commands
- Fetch remote resources
- Compile and evaluate code snippets
- Handle batch prompts concurrently

It supports multiple AI backends (OpenAI, Anthropic/Claude, Ollama, Google Generative API), with robust retry and batching logic.

---

## Table of Contents

1. [Architecture](#architecture)
2. [Features](#features)
3. [Installation](#installation)
4. [Configuration](#configuration)
5. [Usage](#usage)
6. [Examples](#examples)
7. [Advanced Usage](#advanced-usage)
8. [Environment Variables](#environment-variables)
9. [Contribution](#contribution)
10. [Roadmap](#roadmap)
11. [License](#license)

---

## Architecture

RustAIgent follows a modular design:

1. **Core Agent**  
   - Manages conversation state, tool definitions, and dispatch logic.
   - Routes requests to the configured provider (OpenAI, Claude, Ollama, or Google).
2. **Function Calling Layer**  
   - Defines a set of JSON‑schema–based tools (`read_file`, `write_file`, `delete_file`, `list_dir`, `run_command`, `fetch_url`, `eval_code`).
   - Automatically detects and executes tool calls from LLM responses.
3. **Provider Integrations**  
   - **OpenAI**: Chat Completions API with function calling.  
   - **Anthropic (Claude)**: Text completion via the Anthropic API.  
   - **Ollama**: Local LLM endpoint (e.g. `localhost:11434`).  
   - **Google**: Generative Language API (Chat Bison).
4. **Reliability & Scalability**  
   - **Retry Mechanism**: Configurable exponential backoff for failed API calls.  
   - **Batching**: Parallel prompt processing using Tokio tasks and `send_batch_requests`.

---

## Features

- **File I/O**: `read_file(path)`, `write_file(path, content)`, `delete_file(path)`
- **Filesystem Operations**: `list_dir(path)`
- **Shell Execution**: `run_command(command)`
- **HTTP Fetching**: `fetch_url(url)`
- **Code Evaluation**: `eval_code(code)` (stub for secure compilation)
- **Multi‑Provider**: Choose between `openai`, `claude`, `ollama`, `google`
- **Retries & Backoff**: Controlled via `RETRY_COUNT` and `BACKOFF_BASE_MS`
- **Batch Requests**: Process multiple prompts concurrently
- **Customizable**: `MODEL_NAME`, `MAX_TOKENS`, `TEMPERATURE` via env vars

---

## Installation

1. Ensure you have Rust (1.60+) installed. If not, install via [rustup](https://rustup.rs/).
2. Clone the repository:
   ```bash
   git clone https://github.com/makalin/RustAIgent.git
   cd RustAIgent
   ```
3. Build in release mode:
   ```bash
   cargo build --release
   ```
4. The binary will be at `./target/release/RustAIgent`.

---

## Configuration

RustAIgent reads configuration from environment variables (or a `.env` file). See [Environment Variables](#environment-variables).

---

## Usage

Run the agent and interact via standard input:

```bash
./target/release/RustAIgent
```

Switch providers on the fly:

```bash
API_PROVIDER=claude ./target/release/RustAIgent
API_PROVIDER=google ./target/release/RustAIgent
```

During the session, prefix commands to invoke tools explicitly, or let the model choose automatically:

```text
You: read_file("config.toml")
You: run_command("cargo fmt -- --check")
You: fetch_url("https://example.com/data.json")
``` 

---

## Examples

1. **Reading a file**
   ```text
   You: read_file("src/main.rs")
   RustAIgent: [TOOL] -- Contents of src/main.rs...
   ```
2. **Listing directory**
   ```text
   You: list_dir(".")
   RustAIgent: [TOOL] -- src, Cargo.toml, README.md, ...
   ```
3. **Writing to a file**
   ```text
   You: write_file("note.txt", "This is a test.")
   RustAIgent: File written successfully.
   ```
4. **Running a shell command**
   ```text
   You: run_command("ls -la target/release")
   RustAIgent: target binary, debug symbols, etc.
   ```

---

## Advanced Usage

### Batch Processing

Use `send_batch_requests` to handle multiple prompts concurrently in code:

```rust
let prompts = vec!["List files in /etc".into(), "Fetch Rust docs".into()];
let responses = agent.send_batch_requests(prompts).await?;
for msg in responses { println!("Response: {}", msg.content); }
```

### Custom Retry Strategy

Adjust retry parameters in `.env`:

```dotenv
RETRY_COUNT=5
BACKOFF_BASE_MS=200
```

### Custom Model & Temperature

```dotenv
MODEL_NAME=gpt-4o-mini
TEMPERATURE=0.3
MAX_TOKENS=512
```

---

## Environment Variables

| Variable         | Description                                   | Default            |
|------------------|-----------------------------------------------|--------------------|
| `OPENAI_API_KEY` | API key for OpenAI                            | **required**       |
| `GOOGLE_API_KEY` | API key for Google Generative API             | *optional*         |
| `API_PROVIDER`   | `openai`, `claude`, `ollama`, or `google`      | `openai`           |
| `MODEL_NAME`     | Model identifier for provider                 | `gpt-4o-mini`      |
| `MAX_TOKENS`     | Maximum tokens per completion                 | `1024`             |
| `TEMPERATURE`    | Sampling temperature (0.0–1.0)                | `0.7`              |
| `RETRY_COUNT`    | Number of retry attempts on failure           | `3`                |
| `BACKOFF_BASE_MS`| Base backoff duration in ms                   | `500`              |

---

## Contribution

1. Fork this repository
2. Create a new branch: `git checkout -b feature/awesome`
3. Commit your changes: `git commit -am 'Add awesome feature'`
4. Push to the branch: `git push origin feature/awesome`
5. Create a Pull Request

Please follow the Rust style guidelines and include tests where appropriate.

---

## Roadmap

- [ ] Secure sandbox for `eval_code`
- [ ] Enhanced function parameter validation
- [ ] Jittered exponential backoff
- [ ] Support for additional LLM providers
- [ ] Plugin architecture for custom tools

---

## License

This project is licensed under the MIT License. See the [LICENSE](LICENSE) file for details.
