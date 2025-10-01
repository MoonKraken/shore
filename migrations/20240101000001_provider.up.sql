-- Create provider table
CREATE TABLE IF NOT EXISTS provider (
    id INTEGER NOT NULL PRIMARY KEY AUTOINCREMENT,
    name TEXT NOT NULL,
    base_url TEXT NOT NULL,
    disabled BOOLEAN NOT NULL DEFAULT 0,
    deprecated BOOLEAN NOT NULL DEFAULT 0,
    api_key_env_var TEXT NOT NULL,
    created_dt INTEGER NOT NULL
);

-- Insert default providers
INSERT INTO provider (name, base_url, disabled, deprecated, api_key_env_var, created_dt) VALUES 
    ('Hugging Face', 'https://router.huggingface.co/v1', 0, 0, 'HF_TOKEN', strftime('%s', 'now')),
    ('OpenAI', 'https://api.openai.com/v1', 0, 0, 'OPENAI_API_KEY', strftime('%s', 'now')),
    ('Anthropic', 'https://api.anthropic.com/v1', 0, 0, 'ANTHROPIC_API_KEY', strftime('%s', 'now')),
    ('Groq', 'https://api.groq.com/openai/v1', 0, 0, 'GROQ_API_KEY', strftime('%s', 'now'));
