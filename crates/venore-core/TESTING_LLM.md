# Testing the LLM Module

Guide for exercising Venore's LLM module via the CLI examples.

## Prerequisites

1. **Anthropic API key**
   - Get yours at https://console.anthropic.com/
   - Format: `sk-ant-api03-...`

2. **Rust toolchain**
   ```bash
   rustc --version  # 1.70+
   cargo --version
   ```

---

## Step 1: Build the project

From the repo root:

```bash
# Build venore-core (resolves all dependencies)
cargo build -p venore-core

# If something fails, run a check first:
cargo check -p venore-core
```

---

## Step 2: Run the basic tests

### Test 1 — Basic (smoke test)

```bash
# Configure the API key
set ANTHROPIC_API_KEY=sk-ant-api03-YOUR-KEY-HERE

# Run the basic example
cargo run --example test_llm_basic

# Expected output:
# API key found
# API key stored
# LLM Gateway created
# Connection successful
# Generating text with Claude...
# Claude response: [response]
# Test completed successfully
```

**What it covers:**
- Module setup
- Connection to the Anthropic API
- Simple text generation
- Token tracking

---

### Test 2 — Advanced (full feature surface)

```bash
cargo run --example test_llm_advanced
```

**What it covers:**
- Different tasks (Chat, Analysis, Onboarding)
- Different temperatures (0.2 – 0.9)
- Multiple models (Sonnet, Haiku)
- Custom configuration
- System messages
- Per-task token usage

**Expected output:**
```
Venore LLM Module — Advanced Test

----------------------------------------
Test 1: Chat task (high temperature)
----------------------------------------

Response received:
[3 creative names...]
   Tokens: 50 total

----------------------------------------
Test 2: Analysis task (low temperature)
----------------------------------------

Analysis:
[code analysis...]
   Tokens: 75 total

... etc
```

---

### Test 3 — Configuration (SQLite + keyring)

```bash
cargo run --example test_llm_config
```

**What it covers:**
- `DefaultConfigStore` (production path)
- API keys stored in the OS keychain
- Task settings persisted to SQLite
- Configuration round-trip
- Validation
- End-to-end integration

**Expected output:**
```
Venore LLM Module — Configuration Test

Temp directory: C:\Users\...\Temp\venore_test_config
Database: ...\config_test.db

----------------------------------------
Test 1: Create DefaultConfigStore
----------------------------------------

DefaultConfigStore created
Database initialized (migrations applied)

----------------------------------------
Test 2: API key management (keyring)
----------------------------------------

API key stored in OS keychain
Verification: has_key = true
Configured providers: [Anthropic]

... etc
```

---

## Troubleshooting

### Error: `ANTHROPIC_API_KEY not set`

```bash
# Windows
set ANTHROPIC_API_KEY=sk-ant-api03-...

# Linux / macOS
export ANTHROPIC_API_KEY=sk-ant-api03-...
```

### Error: `Failed to connect to database`

Ensure the `~/.venore/` directory exists:

```bash
# Windows
mkdir %USERPROFILE%\.venore

# Linux / macOS
mkdir -p ~/.venore
```

### Error: `No API key configured`

The keyring store needs OS-level permissions:
- **Windows:** active interactive user session.
- **macOS:** may prompt for the keychain password.
- **Linux:** requires `libsecret`.

### Build error: `cannot find crate reqwest`

```bash
cargo clean
cargo build -p venore-core
```

---

## Expected results

### Basic test
- Connection succeeds in < 2 seconds
- A response from Claude is received
- Tokens used: ~20–50

### Advanced test
- All four sub-tests pass
- Different responses across temperatures
- Token usage varies per model

### Configuration test
- SQLite DB created under `%TEMP%\venore_test_config\`
- API key visible in the OS keyring (e.g. Credential Manager on Windows)
- Cleanup runs successfully

---

## Verbose logs

For full traces:

```bash
# DEBUG level
$env:RUST_LOG="debug"
cargo run --example test_llm_basic

# Only venore_core logs
$env:RUST_LOG="venore_core=debug"
cargo run --example test_llm_advanced
```

---

## Validation checklist

Before moving on to the frontend, confirm:

- [ ] `cargo check -p venore-core` passes with no errors
- [ ] `cargo build -p venore-core` compiles successfully
- [ ] `test_llm_basic` runs cleanly
- [ ] `test_llm_advanced` finishes all four sub-tests
- [ ] `test_llm_config` round-trips the configuration
- [ ] API key is stored in the keyring
- [ ] SQLite DB is created under `~/.venore/config.db`
- [ ] Token usage is reported correctly

---

## Next steps

Once the CLI tests pass:

1. **Build venore-desktop**
   ```bash
   cargo build -p venore-desktop
   ```

2. **Run the Tauri app**
   ```bash
   cargo tauri dev
   ```

3. **Exercise from the frontend**
   - Configure the API key from the UI
   - Generate `.context.md` during onboarding
   - Chat with Claude

---

## Getting help

If you hit a wall:

1. Re-run with `RUST_LOG=debug`
2. Verify `ANTHROPIC_API_KEY` is correct
3. Confirm you have an internet connection
4. Check the Anthropic API status: https://status.anthropic.com/

---

## Notes

- Tests use `MockConfigStore` by default (in-memory).
- `test_llm_config` uses `DefaultConfigStore` (keyring + SQLite).
- The examples clean up after themselves.
- API keys are handled safely — they are never logged in full.
