-- Create model table
CREATE TABLE IF NOT EXISTS model (
    id INTEGER NOT NULL PRIMARY KEY AUTOINCREMENT,
    provider_id INTEGER NOT NULL,
    model TEXT NOT NULL,
    api_type INTEGER NOT NULL, -- 0 = OpenAI chat completion compatible, more to come
    disabled BOOLEAN NOT NULL DEFAULT 0,
    deprecated BOOLEAN NOT NULL DEFAULT 0,
    created_dt INTEGER NOT NULL,
    FOREIGN KEY (provider_id) REFERENCES provider(id)
);

-- Insert default model
INSERT INTO model (provider_id, model, api_type, disabled, deprecated, created_dt) VALUES 
    (1, 'Qwen/Qwen3-235B-A22B-Instruct-2507:cerebras', 0, 0, 0, strftime('%s', 'now')),
    (1, 'Qwen/Qwen3-Next-80B-A3B-Instruct:hyperbolic', 0, 0, 0, strftime('%s', 'now')),
    (1, 'deepseek-ai/DeepSeek-V3.1:fireworks-ai', 0, 0, 0, strftime('%s', 'now')),
    (2, 'gpt-5', 0, 0, 0, strftime('%s', 'now')),
    (2, 'gpt-5-mini', 0, 0, 0, strftime('%s', 'now')),
    (2, 'gpt-5-nano', 0, 0, 0, strftime('%s', 'now')),
    (3, 'claude-sonnet-4-5-20250929', 0, 0, 0, strftime('%s', 'now')),
    (3, 'claude-opus-4-1-20250805', 0, 0, 0, strftime('%s', 'now')),
    (3, 'claude-3-5-haiku-20241022', 0, 0, 0, strftime('%s', 'now')),
    (4, 'openai/gpt-oss-20b', 0, 0, 0, strftime('%s', 'now')),
    (4, 'openai/gpt-oss-120b', 0, 0, 0, strftime('%s', 'now')),
    (4, 'moonshotai/kimi-k2-instruct-0905', 0, 0, 0, strftime('%s', 'now')),
    (4, 'qwen/qwen3-32b', 0, 0, 0, strftime('%s', 'now')),
    (4, 'meta-llama/llama-4-scout-17b-16e-instruct', 0, 0, 0, strftime('%s', 'now')),
    (4, 'meta-llama/llama-4-maverick-17b-128e-instruct', 0, 0, 0, strftime('%s', 'now')),
    (4, 'llama-3.1-8b-instant', 0, 0, 0, strftime('%s', 'now')),
    (4, 'groq/compound', 0, 0, 0, strftime('%s', 'now')),
    (4, 'groq/compound-mini', 0, 0, 0, strftime('%s', 'now'));
