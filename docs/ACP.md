# Agent Client Protocol (ACP) Integration

This document describes the current ACP integration in the editor and outlines future improvements.

## Overview

The editor includes full support for the Agent Client Protocol (ACP), enabling integration with AI coding agents like Claude Code. The implementation allows agents to:

- Read and write workspace files
- Stream responses into editor buffers
- Execute tool calls
- Manage conversation history
- Handle permissions per-session

## Quick Start: Using Claude Code

### Prerequisites

1. **Install the ACP Adapter**
   ```bash
   npm install -g @zed-industries/claude-code-acp
   ```

2. **Get an Anthropic API Key**
   - Visit https://console.anthropic.com/
   - Create an API key
   - Set it in your environment:
     ```bash
     export ANTHROPIC_API_KEY=sk-ant-your-key-here
     ```
   - Add this to your shell profile (`~/.bashrc`, `~/.zshrc`, etc.) to make it persistent

3. **Run the Editor**
   ```bash
   cargo run -p the-editor
   ```

### Using ACP Commands

The editor provides three commands for interacting with ACP agents:

1. **`:acp-new-session`** - Creates a new session with Claude Code
   - Opens a new buffer for the conversation
   - Spawns the agent process
   - Associates the buffer with the session

2. **`:acp-send-prompt`** - Sends selected text to the agent
   - Select text in the session buffer
   - Run the command
   - Agent response streams into the buffer in real-time

3. **`:acp-close-session`** - Closes the current session
   - Terminates the agent connection
   - Cleans up session state

### Example Workflow

1. Run `:acp-new-session` to start a conversation
2. Type your question or prompt in the buffer
3. Select the text and run `:acp-send-prompt`
4. Watch the agent's response stream into the buffer
5. Continue the conversation by writing more and sending prompts
6. Run `:acp-close-session` when done

## Current Implementation

### Architecture

The ACP integration consists of several components:

#### 1. **Registry** (`acp/mod.rs`)
- Manages active agents and sessions
- Maps documents to sessions
- Handles async operations via `RegistryHandle`
- Thread-local notification queue for streaming updates

#### 2. **Client** (`acp/client.rs`)
- Implements the `acp::Client` trait
- Handles agent requests (file operations, permissions)
- Routes session notifications to the editor

#### 3. **Session Tracking** (`acp/session.rs`)
- Tracks per-session permissions
- Maintains conversation history
- Associates sessions with document IDs

#### 4. **Commands** (`acp/commands.rs`)
- User-facing commands for session management
- Uses LocalSet job queue for async operations

#### 5. **Job Queue** (`ui/job.rs`)
- Parallel queues for Send and !Send futures
- LocalSet integration for ACP's !Send futures
- Callback system for updating editor state

### Permissions

Currently, sessions have **read-all** access to workspace files by default. Write access is granted per-file as needed. Future improvements will add a permission UI for user approval.

### Streaming

Agent responses stream in real-time:
- Message chunks are queued via `SessionNotification`
- Processed in the render loop (`application.rs`)
- Appended to session documents via transactions
- Special formatting for thinking, tool calls, etc.

## Future Improvements

### 1. Configuration System

**Goal**: Make ACP agents configurable via `config.toml`

**Implementation**:
```toml
[acp]
# Global ACP settings
default-agent = "claude-code"

[[acp.agents]]
name = "claude-code"
command = "claude-code-acp"
auto-start = false

# Environment variables for this agent
[acp.agents.env]
ANTHROPIC_API_KEY = "${ANTHROPIC_API_KEY}"  # Read from shell env

[[acp.agents]]
name = "custom-agent"
command = "/path/to/custom-agent"
args = ["--verbose", "--workspace", "${WORKSPACE}"]
auto-start = false
```

**Changes Needed**:
- Add `ACPConfig` struct in `core/config.rs`
- Parse `[acp]` section from config.toml
- Update `Registry::new()` to accept configs from file
- Support environment variable expansion
- Handle multiple agent configurations

### 2. Keybindings

**Goal**: Add default keybindings for ACP commands

**Suggested Bindings** (Normal mode):
- `<space>an` - ACP new session
- `<space>as` - ACP send prompt
- `<space>ac` - ACP close session

**Implementation**:
- Add to `keymap/default.rs`
- Document in keybindings reference
- Allow customization via config

### 3. Permission UI

**Goal**: Interactive permission approval system

**Features**:
- Popup prompt when agent requests permissions
- Show requested file paths and operations
- Allow/deny/remember choices
- Visual indicators for granted permissions

**Implementation**:
- Implement `request_permission` in `EditorClient`
- Add permission prompt component in `ui/components/`
- Store remembered permissions in session state
- Add visual indicators in statusline

### 4. Better Streaming & UI

**Current Limitations**:
- Messages append as plain text
- No syntax highlighting for code blocks
- No visual distinction between roles
- Thinking/tool calls have basic formatting

**Improvements**:
- Parse markdown in agent responses
- Syntax highlight code blocks
- Visual separators between messages
- Collapsible thinking sections
- Inline tool call results
- Progress indicators for long operations

**Implementation**:
- Add markdown parser for agent responses
- Extend `EditorView` to render ACP-specific UI
- Use different text styles for different message types
- Add metadata to documents for ACP sessions

#### 4.1 Model-Aware Buffer Names

**Current State**: ACP buffers display as `*acp*` (simple static name)

**Goal**: Display the actual model name in the buffer name

**Benefits**:
- Immediately see which model you're talking to
- Distinguish between multiple concurrent sessions with different models
- Better UX when switching between sessions

**Implementation Details**:

The ACP protocol provides model information via `SessionModelState` (requires "unstable" feature):
```rust
pub struct SessionModelState {
  pub current_model_id: ModelId,
  pub available_models: Vec<ModelInfo>,
  pub meta: Option<serde_json::Value>,
}

pub struct ModelInfo {
  pub model_id: ModelId,
  pub name: String,  // Human-readable name like "Claude 3.5 Sonnet"
  pub description: Option<String>,
  pub meta: Option<serde_json::Value>,
}
```

**Steps**:
1. Enable "unstable" feature flag for the ACP crate
2. Store model info in the `Session` struct (currently not stored)
3. Add a custom display name to `Document` or link document to session for dynamic lookup
4. Update `Document::display_name()` to show model name for ACP buffers
5. Handle model changes mid-session (if protocol supports it)

**Display Format Options**:
- `*acp: Claude 3.5 Sonnet*`
- `*claude-sonnet-3.5*`
- `*acp: claude-code*` (fallback to agent name if model unavailable)

**Related Code**:
- `acp/session.rs` - Add model info storage
- `acp/mod.rs` - Extract model info from `NewSessionResponse.models` (line 149)
- `core/document.rs:2192-2203` - Update `display_name()` logic

### 5. Session Persistence

**Goal**: Save and restore conversation history

**Features**:
- Serialize session history to disk
- Load previous sessions on startup
- Session browser/picker
- Export conversations to markdown

**Implementation**:
- Add serialization to `Session` struct
- Store in `~/.config/the-editor/sessions/`
- Load on demand via picker
- Export command for markdown conversion

### 6. Environment Variable Management

**Goal**: Secure API key handling

**Current Issue**: API key must be in shell environment

**Improvements**:
- Support `.env` files in workspace
- Encrypted key storage in config
- Prompt for key if missing
- Per-agent environment isolation

**Implementation**:
- Add `dotenv` crate for `.env` support
- Secure storage using OS keyring
- Runtime prompt for missing keys
- Environment variable isolation per agent

### 7. Agent Status Indicators

**Goal**: Show agent connection status

**Features**:
- Statusline indicator for active sessions
- Connection state (connecting, active, error)
- Message counters (sent/received)
- Agent capabilities display

**Implementation**:
- Add status fields to `Session` struct
- Update statusline component
- Add agent info command
- Visual indicators in buffer gutter

### 8. Multiple Concurrent Sessions

**Current Limitation**: One session per document

**Improvements**:
- Support multiple concurrent sessions
- Session switching via picker
- Broadcast prompts to multiple agents
- Compare responses side-by-side

**Implementation**:
- Modify document→session mapping (one-to-many)
- Add session picker component
- Track active session per view
- Split view support for comparisons

### 9. Custom Slash Commands

**Goal**: Support agent-specific commands

**Features**:
- Register custom commands from agents
- Dynamic command discovery
- Per-agent command namespaces

**Implementation**:
- Parse `AvailableCommandsUpdate` notifications
- Dynamically register commands
- Route to appropriate agent

### 10. MCP Server Integration

**Goal**: Support Model Context Protocol servers

**Features**:
- Configure MCP servers in config.toml
- Pass to agents via `NewSessionRequest`
- Manage MCP server lifecycle

**Implementation**:
- Add MCP config section
- Start/stop MCP servers with sessions
- Pass server info to agents

## Troubleshooting

### Agent Not Found

**Error**: `Failed to spawn claude-code-acp: No such file or directory`

**Solution**:
- Install the adapter: `npm install -g @zed-industries/claude-code-acp`
- Ensure npm global bin is in PATH: `npm config get prefix`
- Add to PATH if needed: `export PATH="$PATH:$(npm config get prefix)/bin"`

### API Key Error

**Error**: `ANTHROPIC_API_KEY environment variable is required`

**Solution**:
- Get an API key from https://console.anthropic.com/
- Set it: `export ANTHROPIC_API_KEY=sk-ant-your-key-here`
- Add to shell profile for persistence

### No Response from Agent

**Symptoms**: Command succeeds but no text appears

**Debugging**:
- Check agent is running: `ps aux | grep claude-code-acp`
- Check logs: The editor outputs to stdout/stderr
- Verify API key is valid
- Try a simple prompt to test connectivity

### Permission Denied

**Error**: Agent can't read/write files

**Solution**:
- Currently, read-all is granted by default
- Write permissions are granted on first write
- Future: Permission UI will allow explicit control

## Architecture Details

### LocalSet Job Queue

ACP futures are `!Send` (can't be sent across threads) because they use thread-local storage. The editor uses a special job queue for these:

- Regular jobs: `tokio::spawn` with Send futures
- Local jobs: `tokio::task::spawn_local` with !Send futures
- Separate channels for each type
- Local callbacks stored in `Rc<RefCell<Vec<LocalCallback>>>`

### Notification Flow

1. Agent sends `SessionNotification` via ACP protocol
2. `EditorClient::session_notification()` receives it
3. Notification pushed to thread-local queue
4. Application render loop drains queue
5. Notifications converted to document updates
6. Text appended via transaction system

### Document Integration

Session documents are regular editor documents with:
- Normal text editing capabilities
- Syntax highlighting (future: for code blocks)
- Undo/redo support
- View management (scrolling, cursor, etc.)

No special document type is needed; the session association is tracked separately in the Registry.

## Contributing

When adding ACP features:

1. **Maintain async boundaries**: Keep ACP operations in LocalSet jobs
2. **Use the job queue**: Don't block the main thread
3. **Handle errors gracefully**: Agents can disconnect or fail
4. **Test with real agents**: The `claude-code-acp` adapter is the reference
5. **Document behavior**: Update this file with new features

## References

- [Agent Client Protocol](https://github.com/agentclientprotocol/agent-client-protocol)
- [Claude Code ACP Adapter](https://github.com/zed-industries/claude-code-acp)
- [Anthropic API](https://docs.anthropic.com/)
- [ACP Rust SDK](https://github.com/agentclientprotocol/rust-sdk)
