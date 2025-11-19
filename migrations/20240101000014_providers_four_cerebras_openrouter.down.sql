-- Add down migration script here
DELETE FROM model where provider_id IN (SELECT id FROM provider WHERE name IN ('OpenRouter', 'Cerebras', 'Local Ollama'));
DELETE FROM provider WHERE name IN ('OpenRouter', 'Cerebras');
