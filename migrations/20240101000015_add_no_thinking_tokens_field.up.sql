-- Add no_thinking_tokens_confirmed field to the model table
ALTER TABLE model ADD COLUMN no_thinking_tokens_confirmed BOOLEAN NOT NULL DEFAULT 0;

-- Update known models that don't produce thinking tokens
-- These are typically older models or models from providers that don't support thinking tokens
UPDATE model SET no_thinking_tokens_confirmed = 1 WHERE model IN (
    'gpt-5', 'gpt-5-mini', 'gpt-5-nano', 'claude-sonnet-4-5-20250929',
    'claude-opus-4-1-20250805', 'claude-3-5-haiku-20241022', 'openai/gpt-oss-20b',
    'openai/gpt-oss-120b', 'moonshotai/kimi-k2-instruct-0905', 'qwen/qwen3-32b',
    'meta-llama/llama-4-scout-17b-16e-instruct', 'meta-llama/llama-4-maverick-17b-128e-instruct',
    'llama-3.1-8b-instant', 'groq/compound', 'groq/compound-mini',
    'MiniMax-M2', 'glm-4-32b-0414-128k', 'claude-haiku-4-5-20251001'
);
