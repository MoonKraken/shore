INSERT INTO model (provider_id, model, api_type, disabled, deprecated, created_dt) VALUES 
    ((SELECT id FROM provider WHERE name = 'MiniMax'), 'MiniMax-M2', 0, 0, 0, strftime('%s', 'now')),
    ((SELECT id FROM provider WHERE name = 'zAI'), 'glm-4.6', 0, 0, 0, strftime('%s', 'now')),
    ((SELECT id FROM provider WHERE name = 'zAI'), 'glm-4.5', 0, 0, 0, strftime('%s', 'now')),
    ((SELECT id FROM provider WHERE name = 'zAI'), 'glm-4.5-air', 0, 0, 0, strftime('%s', 'now')),
    ((SELECT id FROM provider WHERE name = 'zAI'), 'glm-4.5-x', 0, 0, 0, strftime('%s', 'now')),
    ((SELECT id FROM provider WHERE name = 'zAI'), 'glm-4.5-airx', 0, 0, 0, strftime('%s', 'now')),
    ((SELECT id FROM provider WHERE name = 'zAI'), 'glm-4.5-flash', 0, 0, 0, strftime('%s', 'now')),
    ((SELECT id FROM provider WHERE name = 'zAI'), 'glm-4-32b-0414-128k', 0, 0, 0, strftime('%s', 'now')),
    ((SELECT id FROM provider WHERE name = 'Anthropic'), 'claude-haiku-4-5-20251001', 0, 0, 0, strftime('%s', 'now'));