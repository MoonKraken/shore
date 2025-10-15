---
title: Normal Mode
description: Normal mode default key bindings
---
Shore begins in normal mode immediately upon launch. Normal mode is used for perusing chat content, chat history, etc.

### Model Selection & Management

| Key | Description |
|-----|-------------|
| `Ctrl+M` | Open model selection dialog (default models) |
| `Ctrl+m` | Open model selection dialog (current chat models) |
| `Ctrl+p` | Open provider dialog |
| `{` | Move to previous model (with wraparound) |
| `}` | Move to next model (with wraparound) |

### Chat Navigation

| Key | Description |
|-----|-------------|
| `n` | Create new chat |
| `z` | Navigate forward in chat history (supports numeric prefix) |
| `q` | Navigate backward in chat history (supports numeric prefix) |
| `Ctrl+t` | Edit chat title (existing chats only) |

### Navigation When Prompt is Empty

*These keys only work when the prompt input field is empty:*

| Key | Description |
|-----|-------------|
| `0` | Select first model |
| `$` | Select last model |
| `*` | Cycle to next model without pending inference |
| `h` | Move to previous model |
| `l` | Move to next model |
| `j` | Navigate down through message chunks |
| `k` | Navigate up through message chunks |
| `1-9` | Build numeric prefix for commands |

### Copying / Clipboard

| Key | Description |
|-----|-------------|
| `]` | Highlight next chat content item |
| `[` | Highlight previous chat content item |
| `y` | Yank (copy) Highlighted message to clipboard |
| `Esc` | Clear highlight |

### Text Editing

| Key | Description |
|-----|-------------|
| `cc` | Clear textarea and enter insert mode |
| `dd` | Clear textarea without entering insert mode |
| `i` | Enter insert mode |
| `Enter` | Submit message (if prompt not empty) or clear search filter |
| `Esc` | Clear search filter, clear selection, or exit insert mode |

### Chat Management

| Key | Description |
|-----|-------------|
| `x` or `d` | Delete chat (if exists) or clear search filter |

### Search

| Key | Description |
|-----|-------------|
| `/` | Enter search mode |
| `Esc` | Clear search filter (when search is active) |

### System

| Key | Description |
|-----|-------------|
| `Q` | Quit application |

---

*Note: Many commands support numeric prefixes to repeat actions (e.g., `5z` navigates forward 5 chats). Numeric prefixes can only be built when the prompt is empty.*