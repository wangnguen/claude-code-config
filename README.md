# ccc - Claude Code Config CLI

Quick setup tool for Claude Code configuration: API keys, the model each
Claude Code slot uses, and a check that the gateway agrees — from the command
line or a full-screen terminal UI.

## Install

### Windows (PowerShell)

```powershell
irm https://raw.githubusercontent.com/wangnguen/claude-code-config/main/install.ps1 | iex
```

### macOS / Linux

```bash
curl -fsSL https://raw.githubusercontent.com/wangnguen/claude-code-config/main/install.sh | bash
```

### Manual install

Download binary from [Releases](https://github.com/wangnguen/claude-code-config/releases), place it in a folder and add to PATH.

## Quick start

```bash
cd my-project
ccc lite      # asks for your virtual key, checks it, writes .claude/settings.local.json
ccc check     # everything should pass
```

Or do it all from the full-screen UI: `ccc ui`, then follow **Getting started**
on the Home screen.

## Usage

```bash
# Interactive shell: type commands without the `ccc` prefix
ccc

# Full-screen manager: keys, models, config and checks
ccc ui

# Set up this project against the team's LiteLLM gateway (prompts for your key)
ccc lite

# Init .claude config in current project (auto-applies default key)
ccc init
ccc init --base-url https://my-gateway.example.com   # another gateway

# Show current local config
ccc show config

# Show global default config
ccc show global

# Install/update Claude Code permissions in ./.claude/settings.local.json
ccc permission   # or: ccc p

# Check the key, model discovery, and every model slot against the gateway
ccc check

# The model Claude Code starts on in this folder, and why (no network)
ccc models current

# List the models your key can use, and which slots use them
ccc models

# Choose this folder's models: arrow keys and Enter, each choice saved at once
ccc models pick

# Set one slot directly (checked against the gateway first)
ccc models set opus claude-opus-5-vn
ccc models set sonnet 'claude-sonnet-5-vn[1m]'

# Check environment and config status
ccc doctor

# Check for updates and install the newest release
ccc update

# Remove ccc: ~/.ccc, saved keys, and the installer's PATH entry
ccc uninstall

# Show version
ccc version
```

## Updates

`ccc`, `ccc ui` and `ccc version` say when a newer release is out, for example
`Update 0.2.0 is out — run update`. They ask GitHub at most once every 6 hours,
wait at most 2 seconds for the answer, and keep it in
`~/.ccc/update-check.json`. `ccc update` always asks afresh. Set
`CCC_NO_UPDATE_CHECK=1` to turn the notice off.

## Model slots

Claude Code reads one model per slot from `env` in
`.claude/settings.local.json`. `ccc models pick` and `ccc models set` fill them:

| Slot | Variable | What it is |
|---|---|---|
| `main` | `ANTHROPIC_MODEL` | The model a session starts with |
| `sonnet` | `ANTHROPIC_DEFAULT_SONNET_MODEL` | What `sonnet` in `/model` resolves to |
| `opus` | `ANTHROPIC_DEFAULT_OPUS_MODEL` | What `opus` resolves to (also `opusplan`) |
| `haiku` | `ANTHROPIC_DEFAULT_HAIKU_MODEL` | What `haiku` resolves to; also runs background tasks |
| `custom` | `ANTHROPIC_CUSTOM_MODEL_OPTION` | Optional: one extra `/model` entry, any id |

`ANTHROPIC_SMALL_FAST_MODEL` is deprecated in Claude Code. ccc still writes it
for older versions, always equal to the Haiku slot.

Only models the gateway lists for your key are offered, so a picked model
cannot fail with a 403.

### Which model is actually in use

`ccc models current` (and the `Model` line on the splash, `ccc version` and the
TUI Home screen) shows the model a new Claude Code session in this folder starts
on, worked out the way Claude Code decides it:

1. `ANTHROPIC_MODEL`, if any source sets it
2. else the `model` setting, which `/model` saves in `~/.claude/settings.json`
3. else `ANTHROPIC_DEFAULT_MODEL`
4. else Claude Code's default, `opus`

An alias such as `opus` or `sonnet[1m]` is then resolved through its slot. For
each variable, a settings file beats your shell, and the files rank managed
policy, `.claude/settings.local.json`, `.claude/settings.json`, then user
settings. Because `ccc lite` writes `ANTHROPIC_MODEL`, a model saved with
`/model` does not take effect in that folder; `ccc models current` says so.

It reads files only. A `--model` flag or a `/model` switch inside a running
session is not saved anywhere it can see — use `/status` in Claude Code for that.

## Gateway models (`-vn`)

Requests go through a LiteLLM proxy, and each person gets their own virtual API
key. On the team gateway that key is only entitled to the models whose name
ends in `-vn`:

```
claude-opus-5-vn
claude-sonnet-5-vn
claude-haiku-4-5-vn
claude-opus-4-8-vn
claude-sonnet-4-6-vn
```

`ccc models` shows the current list for your key.

The names without the suffix are Claude Code's built-in list. Selecting one
fails with `403 key not allowed to access model` — they are different model
ids, not a permission you can be granted.

What matters, and what does not:

- **Every required slot must be set to a `-vn` model.** An unset slot makes
  the matching `/model` alias fall back to a built-in id, which fails with 403.
- **Model discovery is recommended, not required.**
  `CLAUDE_CODE_ENABLE_GATEWAY_MODEL_DISCOVERY=1` makes `/model` list the
  gateway's real model names. With it off, `/model` shows only the aliases
  (`default`, `sonnet`, `opus`, `haiku`, …) — and those still work, because each
  alias resolves to its slot. `ccc lite` turns discovery on; `g` in the model
  picker toggles it.
- Discovery only lists ids that contain `claude` or `anthropic`. Any other id
  works when set in a slot, but cannot be picked from `/model` by name.

## Other gateways and the model filter

ccc is not tied to the team gateway. The rule for which models count as usable
depends on the gateway:

| Gateway | Filter | Usable models |
|---|---|---|
| The team gateway | `-vn` | Listed models ending in `-vn` |
| Any other | none | Everything the gateway lists for the key |

Override it with `ccc config set model_filter <suffix>`, turn it off with
`ccc config set model_filter none`, and go back to the default with
`ccc config set model_filter auto`.

To point a project at another Anthropic-compatible gateway:

```bash
ccc init --base-url https://my-gateway.example.com
```

ccc then asks that gateway for its models and fits every slot to them. The
gateway must speak the Anthropic Messages API (`/v1/messages`); Anthropic does
not support Claude Code on non-Claude models through a gateway.

## 1M context (`[1m]`)

A model name may carry a `[1m]` marker to select the 1M-context variant:

```
claude-sonnet-5-vn[1m]
claude-opus-5-vn[1m]
```

The marker is client-side. Claude Code parses it off, sends the bare id to the
gateway and turns `[1m]` into the long-context beta header — so `ccc models`
never lists a name with brackets, and a config value like
`claude-sonnet-5-vn[1m]` matching a gateway model `claude-sonnet-5-vn` is
correct, not a mismatch.

Only Sonnet and Opus have a 1M variant. Haiku 4.5 does not, so the Haiku slot
stays bare; ccc refuses `[1m]` on it. In the picker, `1` toggles 1M for the
selected slot.

The marker goes *after* the `-vn` suffix, never before: `claude-sonnet-5-vn[1m]`
is right, `claude-sonnet-5[1m]-vn` is not a model id.

`ccc lite` defaults to `claude-sonnet-5-vn[1m]` for the main and Sonnet slots,
`claude-opus-5-vn[1m]` for Opus, and `claude-haiku-4-5-vn` for Haiku. Before
writing, it checks them against your key's list and swaps any your key cannot
use for the closest model it can.

If the models do not show up, something with higher priority is overriding the
config. In order, highest first: managed policy (`managed-settings.json`),
command-line flags, `.claude/settings.local.json` in the project,
`.claude/settings.json`, then `~/.claude/settings.json`. Shell environment
variables also win over settings files. `ccc doctor` reports the ones it can see.

Note that `~/.ccc/.claude/settings.local.json` is only the template `ccc init`
copies — Claude Code never reads it.

## Full-screen UI

`ccc ui` opens on **Home**; `ccc key` opens on **Keys**; the **Models** screen
is in Home's menu (`ccc models pick` asks the same questions without taking
over the screen). Every screen lists its keys on the bottom line, and `?` shows
all of them with a short explanation. `Esc` goes back, `q` quits.

| Screen | What it does |
|---|---|
| Home | Where this folder stands (key, gateway, models), the four getting-started steps, and the way to each screen |
| Keys | Saved keys, which is the default (`★`) and which this folder uses (`●`) |
| Models | Pick a model for each slot from the gateway's list for the key |
| Config | What Claude Code reads in this folder, secrets masked |
| Check | The same checks as `ccc check` |

**Keys:**

| Key | Action |
|-----|--------|
| `a` | Add a key (name, then the key; input is hidden) |
| `u` | Use the highlighted key in this folder |
| `d` | Make it the default for new folders |
| `m` | Pick this folder's models using this key |
| `s` | Test every saved key against the gateway |
| `n` / `r` | Rename / remove (asks first) |

**Models:**

| Key | Action |
|-----|--------|
| `↑↓` | Move between slots (or models, once inside the list) |
| `→` / `Enter` | Choose a model for the slot |
| `w` / `Ctrl-S` | Save to `.claude/settings.local.json` |
| `f` | Fill empty or unusable slots automatically |
| `1` | 1M context on/off (Sonnet and Opus) |
| `g` | Discovery on/off |
| `e` / `x` | Type / clear the Custom model |
| `r` | Reload the list |

Leaving with unsaved picks asks first. When the models were checked with a key
this folder does not use yet, saving offers to use that key here too.

## Key management from the command line

```bash
# Add a new key (prompts, hidden, when the value is left out)
ccc key add <name> [value]

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
