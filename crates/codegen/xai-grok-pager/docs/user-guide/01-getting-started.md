# Getting Started

> **Community build notice:** This is the unofficial Simplified Chinese
> distribution. Its command is `grok-zh`; it intentionally shares `~/.grok`
> and `GROK_HOME` with the official executable so sessions, credentials, and
> configuration remain identical. The official xAI installer does not install
> or update the `grok-zh` executable.

Grok Build is a terminal-based AI coding assistant from SpaceXAI. It runs as a TUI (Terminal User Interface) that understands your codebase, executes shell commands, edits files, searches the web, and manages tasks.

You can use it interactively as a full-screen TUI, run it headlessly for scripting and CI/CD, or integrate it into editors via the Agent Client Protocol (ACP).

---

## Installation

Community packages are produced by this repository's Releases and documented
preview build pipeline. The upstream `install.sh`,
`install.ps1`, and `@xai-official/grok` package are intentionally not valid
installers for this distribution.

Every `release-v*` archive, including `release-v1.0.13`, has one top-level
directory named after the archive without its `.zip` or `.tar.gz` suffix. Enter
that directory before running the bundled checksum verification and installer.
The plain `v1.0.8` bridge is Windows-only and keeps the legacy flat ZIP layout.

Verify the installation:

```bash
grok-zh --version
```

Updater-enabled builds accept only the exact platform archive and checksum
sidecar metadata from immutable Releases in this repository. They verify the
GitHub SHA-256, safe archive layout, and the package's inner `SHA256SUMS.txt`,
and never fall back to official xAI release channels. Background downloads are
off by default: startup checks metadata and shows a notice, while `Ctrl+U`
authorizes that one download and install. Windows `v1.0.3` and `v1.0.5`
builds update automatically through the Windows-only `v1.0.8` bridge before
selecting a modern `release-v*` version. The much older
`v1.0.0-zh.preview.3`, which hardcoded the former repository, requires one
manual complete-package installation.

To fetch a repository through Grove (NFS on macOS, FUSE on Linux) after
`[clone] enabled = true` in Grove config:

```bash
grok clone <url> [dir]
```

The default is a depth-1 checkout of the selected branch. Pass `--full-history`
for a complete clone. See [grok clone](27-grok-clone.md).

---

## First Launch

Start Grok by running:

```bash
grok-zh
```

On first launch, Grok opens your browser to authenticate with grok.com. After you sign in, Grok stores your credentials in `~/.grok/auth.json`, where they persist across sessions and are shared by `grok` and `grok-zh`. Grok refreshes your credentials automatically and prompts you to sign in again when they can no longer be renewed.

If you prefer API key authentication (e.g., for CI/CD or environments without a browser), set the `XAI_API_KEY` environment variable instead:

```bash
export XAI_API_KEY="xai-..."
grok-zh
```

See [Authentication](02-authentication.md) for the full set of auth options including OIDC, external auth providers, and device code flow.

---

## Basic Interaction

Once authenticated, Grok presents a full-screen TUI with two main areas:

- **Scrollback** -- the conversation history showing your prompts, Grok's responses, tool calls, file edits, and more.
- **Prompt** -- the input area at the bottom where you type messages.

Type a message and press `Enter` to send it. Grok reads files, runs commands, and edits code as needed. Each tool run streams into the scrollback in real time.

Press `Tab` to move focus between the prompt and the scrollback. While a turn is running, `Esc` cancels it (the exception is fullscreen vim scrollback mode, where mid-turn `Esc` is a no-op; minimal mode cancels even with vim on); `Ctrl+C` cancels once the composer is empty — with a draft, the first press only clears it. Idle, press `Esc` twice within 800ms to clear a non-empty prompt, or (with an empty prompt and conversation messages) to open rewind — see [Keyboard Shortcuts](03-keyboard-shortcuts.md#escape). With the scrollback focused, use the arrow keys to select entries and to collapse or expand them. To navigate with `j`/`k` and fold with `h`/`l` instead, enable Vim mode.

### File References

Use `@` in your prompt to attach files:

```
@src/main.rs              # Attach a file
@src/main.rs:10-50        # Attach lines 10-50
@src/                     # Browse a directory
```

The `@` operator opens a fuzzy file picker. By default it respects `.gitignore` and hides dotfiles. Prefix with `!` to search hidden files:

```
@!.github                 # Search hidden files
@!.env                    # Attach a .env file
```

### Permissions

By default, Grok asks for permission before executing shell commands or editing files. You can approve individually or toggle always-approve mode:

- Press `Ctrl+O` to toggle always-approve mode
- Use the `--yolo` flag at launch: `grok-zh --yolo`
- Type `/always-approve` in the prompt to toggle the mode

---

## Key Concepts

### Sessions

Every conversation is a **session**. Sessions are automatically saved to `~/.grok/sessions/`, shared by `grok` and `grok-zh`, and can be resumed later. Each session tracks the full conversation history, tool calls, file edits, and task state.

- Start a new session: `Ctrl+N` or `/new`
- Resume a previous session: `/resume` in the TUI, or `--resume <ID>` from the CLI
- Continue the most recent session: `grok-zh -c`

### Scrollback

The scrollback is the main display area. It shows:

- **User prompts** -- your messages, rendered as sticky headers
- **Agent messages** -- Grok's responses with full markdown rendering and syntax highlighting
- **Thinking blocks** -- Grok's reasoning process (collapsible)
- **Tool calls** -- file edits (with inline diffs), command executions, search results, and more
- **Task lists** -- TODO items tracking progress

Collapse or expand the selected entry with the `Left`/`Right` arrow keys (or `h`/`l` and `e` in Vim mode). In Vim mode, press `y` to copy its content and `Y` to copy its metadata (for example, the command that ran). Press `Enter` to open it in the fullscreen viewer (in any mode).

### Tools

Grok has built-in tools for:

| Tool | Description |
|------|-------------|
| `read_file` / `search_replace` | Read and edit files with line-precise changes |
| `grep` | Regex search across your codebase (powered by ripgrep) |
| `list_dir` | List directory contents |
| `run_terminal_command` | Execute shell commands |
| `web_search` / `web_fetch` | Search the web and fetch URLs |
| `todo_write` | Create and manage task lists |
| `spawn_subagent` | Spawn parallel subagent sessions |
| `memory_search` | Search cross-session memory |

Tools can be extended with [MCP servers](05-configuration.md#mcp-servers) for integrations like GitHub, databases, and more.

### Slash Commands

Type `/` in the prompt to access commands. These provide quick actions without writing a full prompt:

```
/model grok-4.6                 # Switch model
/compact                          # Compress conversation history
/always-approve                   # Toggle always-approve mode
/new                              # Start a new session
```

See [Slash Commands](04-slash-commands.md) for the complete reference.

---

## Common Launch Options

```bash
# Launch the interactive TUI and submit an initial prompt as the first turn
grok-zh "fix the failing auth test and run it"

# Initial prompt in a new git worktree. Use --worktree=<name> (with `=`) so the
# prompt isn't swallowed as the worktree name — `grok-zh -w "refactor module X"`
# would treat "refactor module X" as the worktree label, not the prompt.
grok-zh --worktree=feat "refactor module X"

# Base the worktree on a specific branch (e.g. main) instead of the current HEAD:
grok-zh -w --ref main "implement feature from main"


# Start in a specific project directory
grok-zh --cwd ~/projects/my-app

# Add project-specific rules
grok-zh --rules "Always use TypeScript. Prefer functional components."

# Auto-approve all tool executions
grok-zh --yolo

# Use a specific model
grok-zh -m grok-build

# Resume a previous session
grok-zh --resume <session-id>

# Continue the most recent session
grok-zh -c

# Experimental scrollback-native render mode. Sticky: plain `grok-zh` reopens in
# the mode last chosen via --minimal/--fullscreen (or /minimal//fullscreen).
grok-zh --minimal

# Back to the standard fullscreen TUI (and make it sticky again)
grok-zh --fullscreen

# Headless mode (for scripts)
grok-zh -p "Explain this codebase"
```

---

## Headless Mode

Run Grok non-interactively for scripting, CI/CD, and automation:

```bash
grok-zh -p "Your prompt here"
```

Output formats:

| Format | Flag | Description |
|--------|------|-------------|
| `plain` | (default) | Human-readable text |
| `json` | `--output-format json` | Single JSON object with `text`, `stopReason`, `sessionId`, and `requestId` |
| `streaming-json` | `--output-format streaming-json` | NDJSON event stream for real-time processing |

Example CI/CD usage:

```bash
grok-zh -p "Review changes for bugs" --output-format json --yolo | jq -r '.text'
```

---

## Project Rules (AGENTS.md)

Add per-project instructions by creating an `AGENTS.md` file in your repository. Grok reads these files and injects their contents as a project-instructions message at the start of the conversation:

```
~/.grok/AGENTS.md           # Global rules (apply to all projects)
<repo-root>/AGENTS.md       # Repository-level rules
<cwd>/AGENTS.md             # Directory-level rules (highest priority)
```

Deeper files take precedence. Grok also reads `CLAUDE.md` files for compatibility.

---

## Where to Go Next

| Document | What You Will Learn |
|----------|-------------------|
| [Authentication](02-authentication.md) | Browser login, API keys, OIDC, external auth, device code flow |
| [Keyboard Shortcuts](03-keyboard-shortcuts.md) | Complete reference for all key bindings |
| [Slash Commands](04-slash-commands.md) | All available `/` commands |
| [Configuration](05-configuration.md) | config.toml, pager.toml, environment variables |
