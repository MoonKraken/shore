---
title: About Shore
description: An overview of Shore
---

Shore is a terminal based chatbot interface that is simultaneously minimalist, productive, and aesthetically pleasing.

### Core Features
* **Terminal based with vim inspired key bindings** - no need to leave your terminal for LLM chats
* **Chat concurrently with multiple models and providers** - Create a new chat with one specific model, or with 20 different models spanning multiple LLM providers
* **Free and open source** - specific license TBD
* **Support for most major LLM providers** (see below) - eligible providers automatically detected based on API keys present in shell environment variables
* **Locally stored** chat history and settings are all stored locally
* **Full text search** - quickly review learnings from prior conversations before submitting new prompts
* **Blazingly Fast** - written 100% in Rust 🦀

### Terminal based with vim inspired bindings
Use `n` to create a new chat and put the prompt field into insert mode. The prompt entry field behaves like a vim buffer. Press `Esc` to return to `normal` mode. Use `b` and `w` to traverse words. `cc` deletes the entire line and puts you back in insert mode. Use `h` and `l` to switch between models associated with the current chat. `j` and `k` traverse down and up in the current chat messages, respectively. These are just a few examples - see [key bindings](/keybindings/overview) for a comprehensive list.

### Concurrent Chat
Use one model for a traditional chat experience. Add additional models to a chat to send prompts all models concurrently. Use `h` and `l` to cycle through the responses of each model.

### Free and open source
Contributions are welcome! [Shore GitHub Repository](https://github.com/MoonKraken/shore)

### Supported LLM Providers
* Hugging Face
* OpenAI
* Anthropic
* Groq

### Locally Stored
All conversation history, chat profiles etc are stored in a local SQLite database.  default location is `~/.shore/default.db`

### Full Text Search
In normal mode, use `/` to enter search mode. Search terms will be matched on both titles and content of your chat history.
