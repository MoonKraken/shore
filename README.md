# Shore

A Terminal User Interface (TUI) application for chatting with multiple language models, built with Rust and Ratatui.

## Features

- **Multi-LM Support**: Chat with multiple language models simultaneously (OpenAI, Anthropic, Groq, Hugging Face)
- **Chat History**: Persistent chat history stored in local SQLite database
- **Vim-like Navigation**: Familiar key bindings for navigation and text editing
- **Collapsible Panes**: Organized interface with chat history, content, and prompt input panes
- **Database Management**: Create and switch between multiple database files
- **Focus Management**: Navigate between different interface components with keyboard shortcuts

## Requirements

- Rust 1.70 or later
- Cargo

## Installation

1. Clone the repository:
```bash
git clone <repository-url>
cd shore
```

2. Build the application:
```bash
cargo build --release
```

3. (Optional) Install system-wide:
```bash
cargo install --path .
```

## Configuration

Shore uses environment variables for API keys:

```bash
export OPENAI_API_KEY="your-openai-api-key"
export ANTHROPIC_API_KEY="your-anthropic-api-key"
export GROQ_API_KEY="your-groq-api-key"
export HF_API_KEY="your-huggingface-api-key"
```

## Usage

### Running Shore

```bash
# Run with default database
cargo run

# Run with a specific database
cargo run -- -d mydatabase
```

### Key Bindings

#### Global
- `q` - Quit the application
- `n` - Create a new chat
- `i` - Enter insert mode (focus prompt input)
- `Ctrl+h` - Focus chat history pane
- `Ctrl+j` - Focus prompt input pane
- `Ctrl+k` - Focus chat content pane

#### Chat History Pane
- `j/k` - Navigate up/down through chat history
- `[n]j/[n]k` - Move n chats up/down (e.g., `2j` moves 2 chats down)
- `/`, `?`, `f`, `s` - Open search modal
- `S` - Clear keyword filter
- `r` - Open date range filter modal
- `R` - Clear date range filter
- `x` - Clear all filters

#### Chat Content Pane
- `j/k` - Navigate up/down through messages
- `h/l` - Cycle through different model responses (when multiple models are enabled)
- `y` - Copy highlighted message to clipboard
- `Enter` - Enter read-only view mode for highlighted message
- `Escape` - Exit read-only view mode

#### Prompt Input Pane
- `i` - Enter insert mode
- `Escape` - Exit insert mode
- `Enter` - Submit message (when in insert mode)
- Standard vim-like text editing commands when in insert mode

#### Model and Tool Management
- `Ctrl+M` - Set default models for new chats
- `Ctrl+m` - Set models for current chat (before first message)
- `Ctrl+T` - Set default tools for new chats
- `Ctrl+t` - Set tools for current chat

#### Database Management
- `Ctrl+d` - Open database selection modal

#### Chat Management
- `t` - Edit chat title (when in chat history or content pane)
- `Ctrl+g` - Auto-generate chat title

## Database Storage

Shore stores all data in SQLite databases located in `~/.shore/`:
- `~/.shore/default.db` - Default database
- `~/.shore/[name].db` - Named databases

### Database Schema

- **provider** - Available AI providers (OpenAI, Anthropic, etc.)
- **model** - Available models for each provider
- **tool** - Available CLI tools for model interactions
- **chat** - Individual chat sessions
- **chat_model** - Many-to-many relationship between chats and models
- **chat_message** - Individual messages within chats

## Development

### Project Structure

```
src/
├── main.rs          # Application entry point
├── app.rs           # Core application state and logic
├── database.rs      # Database operations and migrations
├── models.rs        # Data models and types
├── ui.rs            # User interface rendering
├── handlers.rs      # Event handlers
└── api.rs           # API client integrations
```

### Building from Source

```bash
# Debug build
cargo build

# Release build
cargo build --release

# Run tests
cargo test

# Run with logging
RUST_LOG=debug cargo run
```

## Architecture

Shore is built with:
- **Ratatui** - Terminal user interface framework
- **SQLx** - Async SQL toolkit with compile-time query validation
- **Tokio** - Async runtime
- **Crossterm** - Cross-platform terminal manipulation
- **tui-textarea** - Vim-like text area widget

The application follows a clean architecture pattern with separated concerns:
- UI rendering is handled by the `ui` module
- Business logic is contained in the `app` module
- Database operations are abstracted in the `database` module
- API integrations are centralized in the `api` module

## Contributing

1. Fork the repository
2. Create a feature branch
3. Make your changes
4. Add tests if applicable
5. Submit a pull request

## License

[Add your license information here]