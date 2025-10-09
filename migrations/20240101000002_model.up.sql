-- Create model table
CREATE TABLE IF NOT EXISTS model (
    id INTEGER NOT NULL PRIMARY KEY AUTOINCREMENT,
    provider_id INTEGER NOT NULL,
    model TEXT NOT NULL,
    disabled BOOLEAN NOT NULL DEFAULT 0,
    deprecated BOOLEAN NOT NULL DEFAULT 0,
    created_dt INTEGER NOT NULL,
    FOREIGN KEY (provider_id) REFERENCES provider(id)
);

-- Insert default model
INSERT INTO model (provider_id, model, disabled, deprecated, created_dt) VALUES 
    (1, 'Qwen/Qwen3-235B-A22B-Instruct-2507:cerebras', 0, 0, strftime('%s', 'now')),
    (1, 'Qwen/Qwen3-Next-80B-A3B-Instruct:hyperbolic', 0, 0, strftime('%s', 'now')),
    (1, 'deepseek-ai/DeepSeek-V3.1:fireworks-ai', 0, 0, strftime('%s', 'now')),
    (2, 'gpt-5', 0, 0, strftime('%s', 'now')),
    (2, 'gpt-5-mini', 0, 0, strftime('%s', 'now')),
    (2, 'gpt-5-nano', 0, 0, strftime('%s', 'now')),
    (2, 'gpt-5-pro', 0, 0, strftime('%s', 'now'));
