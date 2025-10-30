-- Add down migration script here
DELETE from provider where name in ('MiniMax', 'zAI');