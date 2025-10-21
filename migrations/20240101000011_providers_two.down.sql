-- Delete models and providers by name (robust to autoincrement changes)
DELETE FROM model WHERE provider_id IN (SELECT id FROM provider WHERE name IN ('xAI', 'Perplexity'));
DELETE FROM provider WHERE name IN ('xAI', 'Perplexity');