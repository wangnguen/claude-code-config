# ccc - Claude Code Config CLI

Quick setup tool for Claude Code configuration, with a built-in interactive TUI key manager.

## Install

### Windows (PowerShell)

```powershell
irm https://raw.githubusercontent.com/wangnguen/ccc/main/install.ps1 | iex
```

### macOS / Linux

```bash
curl -fsSL https://raw.githubusercontent.com/wangnguen/ccc/main/install.sh | bash
```

### Manual install

Download binary from [Releases](https://github.com/wangnguen/ccc/releases), place it in a folder and add to PATH.

## Usage

```bash
# Set up this project against the LiteLLM gateway (prompts for your virtual key)
ccc lite

# Init .claude config in current project (auto-applies default key)
ccc init

# Show current local config
ccc show config

# Show global default config
ccc show global

# Install/update Claude Code permissions in ./.claude/settings.local.json
ccc permission   # or: ccc p

# Check API connection, model discovery, and pinned models
ccc check

# List the models your key can reach on the gateway
ccc models

# Check environment and config status
ccc doctor

# Check for updates
ccc update

# Show version
ccc version
```

## Gateway models (`-vn`)

Requests go through a LiteLLM proxy, and each person gets their own virtual API
key. That key is only entitled to the models whose name ends in `-vn`:

```
claude-opus-5-vn
claude-sonnet-5-vn
claude-haiku-4-5-vn
claude-opus-4-8-vn
claude-sonnet-4-6-vn
```

The names without the suffix are Claude Code's built-in list. Selecting one
fails with `403 key not allowed to access model` — they are different model
ids, not a permission you can be granted.

Two consequences worth knowing:

- `CLAUDE_CODE_ENABLE_GATEWAY_MODEL_DISCOVERY=1` is required, otherwise the
  model picker only offers built-ins. `ccc lite` writes it for you.
- Every `ANTHROPIC_*MODEL` variable must be pinned to a `-vn` name, including
  `ANTHROPIC_SMALL_FAST_MODEL`. Leaving one unset is not safe: Claude Code then
  falls back to a built-in id and fails the same way.

`ccc lite` defaults to `claude-sonnet-5-vn` for the main and Sonnet slots,
`claude-opus-5-vn` for Opus, and `claude-haiku-4-5-vn` for the fast slot.

Run `ccc check` to verify all of this at once, and `ccc models` to see the list.

If `-vn` models do not appear, something with higher priority is overriding the
config. In order, highest first: managed policy (`managed-settings.json`),
command-line flags, `.claude/settings.local.json` in the project,
`.claude/settings.json`, then `~/.claude/settings.json`. Shell environment
variables also win over settings files. `ccc doctor` reports the ones it can see.

Note that `~/.ccc/.claude/settings.local.json` is only the template `ccc init`
copies — Claude Code never reads it.

### Key Management

#### Interactive TUI (recommended)

```bash
ccc key
```

Opens a full-screen terminal UI with:

- **Key table** — navigate with `↑↓` or `j/k`, default key marked with `★`
- **Modal dialogs** — inline input for add/rename, confirmation for remove
- **Status dashboard** — full-screen view with progress bar, API info, and live results
- **Toast notifications** — instant feedback for all operations

**Keyboard shortcuts:**

| Key | Action |
|-----|--------|
| `a` | Add a new key (modal input) |
| `d` | Set highlighted key as default |
| `u` | Use highlighted key for current folder |
| `r` | Remove highlighted key (with confirmation) |
| `n` | Rename highlighted key (modal input) |
| `s` | Check all keys status (full-screen dashboard) |
| `q` / `Esc` | Quit |

#### CLI commands

All key operations are also available as direct CLI commands:

```bash
# Add a new key
ccc key add <name> <value>

# List all saved keys
ccc key list

# Set default key (saved in keys.json, used by ccc init)
ccc key default [name]

# Use a key for current folder (.claude/settings.local.json)
ccc key use [name]

# Remove a key
ccc key remove [name]

# Rename a key
ccc key rename

# Check all keys status (test API connection)
ccc key status
```

**default** vs **use**:
- `ccc key default` — sets which key is the global default (stored in `~/.ccc/keys.json`). Used automatically when running `ccc init`.
- `ccc key use` — applies a key to the current project folder (writes to `.claude/settings.local.json`).

