-- this flag is true if we want to fetch available models
-- from the provider's models API, as opposed to curating them ourselves
-- curation is nice because functionality can be verified
-- but is unrealistic for providers like OpenRouter
ALTER TABLE provider ADD COLUMN models_from_list BOOLEAN NOT NULL DEFAULT false;

-- this flag is true in cases where we should hit the models API on startup before considering
-- the provider available (local providers like Ollama are the primary case this is needed)
ALTER TABLE provider ADD COLUMN availability_requires_models_response BOOLEAN NOT NULL DEFAULT false;

-- last time we checked the models api to get new and deprecated models
-- this will be updated by the application
ALTER TABLE provider ADD COLUMN last_models_update_timestamp INTEGER NOT NULL DEFAULT 0;

-- how often we should check the models api to get new and deprecated models.
-- this is provider specific. 0 indicates we should refresh every time shore starts.
ALTER TABLE provider ADD COLUMN models_refresh_interval_seconds INTEGER NOT NULL DEFAULT 86400;

-- we made the api key column not nullable, so local llama uses an empty string
-- to indicate no api key is required
INSERT INTO provider (name, base_url, disabled, deprecated, api_key_env_var, created_dt, models_from_list, availability_requires_models_response, last_models_update_timestamp, models_refresh_interval_seconds) VALUES
    ('OpenRouter', 'https://api.openrouter.ai/v1', 0, 0, 'OPENROUTER_API_KEY', strftime('%s', 'now'), true, false, 0, 86400),
    ('Cerebras', 'https://api.cerebras.ai/v1', 0, 0, 'CEREBRAS_API_KEY', strftime('%s', 'now'), true, false, 0, 86400),
    ('Local Ollama', 'http://localhost:11434/v1', 0, 0, '', strftime('%s', 'now'), true, true, 0, 0);
