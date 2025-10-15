---
title: Setup & Installation
description: How to install Shore
---
### API Keys
Shore reads provider API keys on launch from its shell environment. To submit prompts, at least one of these must be set:

| Provider | Environment Variable |
|----------|---------------------|
| Hugging Face | `HF_TOKEN` |
| OpenAI | `OPENAI_API_KEY` |
| Anthropic | `ANTHROPIC_API_KEY` |
| Groq | `GROQ_API_KEY` |

Example:
```zsh
export HF_TOKEN=[your token here] 
```

### With Cargo
```zsh
cargo install shore
```
### macOS

Using [Homebrew](https://brew.sh/):
```zsh
TODO update this
brew install tap/shore/whatever
```