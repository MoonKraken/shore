![Shore Terminal Interface](shore-hero.png)

# Shore

Shore is a terminal based chatbot interface that is simultaneously minimalist, productive, and aesthetically pleasing. [Read the docs](https://moonkraken.github.io/shore/gettingstarted/about/)

Conversations are stored locally in a SQLite database, by default in `~/.shore/default.db`

[Watch Usage Video on YouTube](https://youtu.be/UAK6dQbnknE)

## Installation
1. `cargo install shore`
1. Set **at least** one API key
	```shell
	export HF_TOKEN=[your token here] # Hugging Face
	export OPENAI_API_KEY=[your token here] # OpenAI
	export ANTHROPIC_API_KEY=[your token here] # Anthropic
	export GROQ_API_KEY=[your token here] # Groq
	export XAI_API_KEY=[your token here] # xAI (Grok)
	export SONAR_API_KEY=[your token here] # Perplexity
	export MINIMAX_API_KEY=[your token here] # MiniMax
	export ZAI_API_KEY=[your token here] # zAI
	```
1. Review [Keybindings](https://moonkraken.github.io/shore/keybindings/01-overview/)
