-- Delete providers and their associated models that were added in the up migration
DELETE FROM chat_profile_model WHERE model_id IN (SELECT id FROM model WHERE provider_id IN (SELECT id FROM provider WHERE name IN ('OpenRouter', 'Cerebras', 'Local Ollama')));
DELETE FROM chat_model WHERE model_id IN (SELECT id FROM model WHERE provider_id IN (SELECT id FROM provider WHERE name IN ('OpenRouter', 'Cerebras', 'Local Ollama')));
DELETE FROM chat_message WHERE model_id IN (SELECT id FROM model WHERE provider_id IN (SELECT id FROM provider WHERE name IN ('OpenRouter', 'Cerebras', 'Local Ollama')));
DELETE FROM model WHERE provider_id IN (SELECT id FROM provider WHERE name IN ('OpenRouter', 'Cerebras', 'Local Ollama'));
DELETE FROM provider WHERE name IN ('OpenRouter', 'Cerebras', 'Local Ollama');

-- Drop the columns that were added to the provider table in the up migration
ALTER TABLE provider DROP COLUMN models_from_list;
ALTER TABLE provider DROP COLUMN availability_requires_models_response;
ALTER TABLE provider DROP COLUMN last_models_update_timestamp;
ALTER TABLE provider DROP COLUMN models_refresh_interval_seconds;
