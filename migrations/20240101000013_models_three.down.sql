-- Delete models by provider name and model name (robust to autoincrement changes)
DELETE FROM model WHERE provider_id = (SELECT id FROM provider WHERE name = 'MiniMax') AND model = 'MiniMax-M2';
DELETE FROM model WHERE provider_id = (SELECT id FROM provider WHERE name = 'zAI') AND model IN ('glm-4.6', 'glm-4.5', 'glm-4.5-air', 'glm-4.5-x', 'glm-4.5-airx', 'glm-4.5-flash', 'glm-4-32b-0414-128k');
DELETE FROM model WHERE provider_id = (SELECT id FROM provider WHERE name = 'Anthropic') AND model = 'claude-haiku-4-5-20251001';

