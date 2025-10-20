---
title: Overview / Essentials
description: Overview of Shore key bindings, including the different modes
---
Like vim, Shore has a concept of modes. Key behavior may differ depending on the current mode.

Other state may affect key behavior as well. For example, if the prompt input field is not empty `hjkl` will move the caret around in it. Otherwise, `h` and `l` cycle through the models associated with the current chat, while `j` and `k` move down and up amongst the messages of the current chat.

Below are the most essential key bindings, roughly in order of importance. The pages following have a more comprehensive coverage of bindings by mode, but get familiar with these first.

### Essential Key Bindings

| Key | Description |
|-----|-------------|
| `n` | Create new chat and enter prompt insert mode |
| `Q` | Quit |
| `Esc` | Exit insert mode |
| `Enter` | Submit prompt |
| `Ctrl-m` | Edit model selection and order for current chat **only works when current chat has no messages** |
| `Ctrl-M` | Edit default model selection and order |
| `{` / `}` | Cycle through models associated with the current chat |
| `h` / `l` | Cycle through models associated with the current chat (if prompt field is empty) |
| `j` / `k` | Move up and down amongst messages in the current chat (if prompt field is empty) |
| `q` / `z` | Move up and down in chat history (supports numeric prefix for multi-line jumps) |
| `/` | Search mode (full text search of chat content and titles) |