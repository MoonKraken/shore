---
title: Normal Mode
description: Normal mode default key bindings
---
Shore begins in normal mode immediately upon launch. Normal mode is used for perusing chat content, chat history, etc.

### System
| Key | Description |
|-----|-------------|
| `Q` | Quit to terminal |

### Model and Provider Management
| Key | Description |
|-----|-------------|
| `Ctrl+M` | Open model selection dialog (default models) |
| `Ctrl+m` | Open model selection dialog (current chat models) |
| `Ctrl+p` | Open provider dialog |

### Chat History
| Key | Description |
|-----|-------------|
| `z` | Navigate forward in chat history (supports numeric prefix) |
| `q` | Navigate backward in chat history (supports numeric prefix) |
| `1-9` | Prior to pressing `z` or `q`, specify number of lines to go up or down |
| `Ctrl-h` | Show/Hide Chat History |

### Chat
| Key | Description |
|-----|-------------|
| `n` | Create new chat |
| `Ctrl+t` | Edit chat title (doesn't work prior to first prompt) |
| `{` | Move to previous model (with wraparound) |
| `}` | Move to next model (with wraparound) |
| `]` | Highlight next chat content item |
| `[` | Highlight previous chat content item |
| `y` | Yank (copy) Highlighted message to clipboard |
| `cc` | Clear textarea and enter insert mode |
| `i` | Enter insert mode |
| `Enter` | Submit message (if prompt not empty) or clear search filter |
| `Esc` | Clear search filter, clear selection, or exit insert mode |

### Prompt Text Field
When in insert mode, most all keystrokes are sent to the prompt text field. Many vim motions are available.
**Refer to the [Edtui readme](https://github.com/preiter93/edtui/blob/main/README.md) for all the prompt field editor keybindings available**

### Chat (when prompt field is empty)
*When the prompt field is **not** empty, these control the prompt text editor. Otherwise:*
| Key | Description |
|-----|-------------|
| `0` | Select first model |
| `$` | Select last model |
| `*` | Cycle to next model without pending inference |
| `h` | Move to previous model |
| `l` | Move to next model |
| `j` | Navigate down through message chunks |
| `k` | Navigate up through message chunks |
| `gg` | Navigate to the first message of the current chat |
| `G` | Navigate to the last message of the current chat |
| `1-9` | Build numeric prefix for commands |
| `x` or `d` | Delete chat (if exists) or clear search filter |

### Search
| Key | Description |
|-----|-------------|
| `/` | Enter search mode |
| `Esc` | Clear search filter (when search is active) |

---

*Note: Many commands support numeric prefixes to repeat actions (e.g., `5z` navigates forward 5 chats). Numeric prefixes can only be built when the prompt is empty.*