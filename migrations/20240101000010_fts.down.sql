-- Drop triggers
DROP TRIGGER IF EXISTS chat_message_au;
DROP TRIGGER IF EXISTS chat_message_ad;
DROP TRIGGER IF EXISTS chat_message_ai;
DROP TRIGGER IF EXISTS chat_au;
DROP TRIGGER IF EXISTS chat_ad;
DROP TRIGGER IF EXISTS chat_ai;

-- Drop FTS tables
DROP TABLE IF EXISTS chat_message_fts;
DROP TABLE IF EXISTS chat_fts;

