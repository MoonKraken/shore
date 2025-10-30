-- Add up migration script here
INSERT INTO provider (name, base_url, disabled, deprecated, api_key_env_var, created_dt) VALUES 
    ('MiniMax', 'https://api.minimax.io/v1', 0, 0, 'MINIMAX_API_KEY', strftime('%s', 'now')),
    ('zAI', 'https://api.z.ai/api/paas/v4', 0, 0, 'ZAI_API_KEY', strftime('%s', 'now'));