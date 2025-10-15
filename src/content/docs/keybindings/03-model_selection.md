---
title: Model Selection
description: Key bindings for the model selection dialog
---
The model selection dialog is used to select models *and* specify an order for the selected models. This can either be for the default chat profile (the dialog was opened with `Ctrl-M`) or for the current chat (dialog was opened with `Ctrl-m`).

Like the main application, the model selection dialog has multiple modes: Normal, Search, and Visual.

## Normal Mode

| Key | Description |
|-----|-------------|
| `j` / `k` | Move selection down / up (supports numeric prefix) |
| `gg` | Go to first item |
| `G` | Go to last item |
| `l` / `h` / `Space` / `Enter` | Toggle selected model on/off |
| `Ctrl-j` | Move selected enabled model down in order |
| `Ctrl-k` | Move selected enabled model up in order |
| `v` | Enter visual mode |
| `/` | Enter search mode |
| `x` / `q` / `c` / `d` | Clear search string |
| `Esc` | Clear search if present, otherwise apply selection and close dialog |
| `0-9` | Accumulate numeric prefix for movement commands |

## Search Mode

| Key | Description |
|-----|-------------|
| Any character | Add character to search query |
| `Backspace` | Remove last character from search query |
| `Enter` | Exit search mode, return to normal mode |
| `Esc` | Clear search query and return to normal mode |

## Visual Mode

| Key | Description |
|-----|-------------|
| `j` | Move selection down (extends visual selection) |
| `k` | Move selection up (extends visual selection) |
| `l` / `h` / `Space` / `Enter` | Toggle all models in visual selection range |
| `Esc` / `v` | Exit visual mode, return to normal mode |