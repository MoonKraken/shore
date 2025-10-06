-- Create FTS5 virtual tables for full text search

-- FTS for chat titles
CREATE VIRTUAL TABLE IF NOT EXISTS chat_fts USING fts5(
    title,
    content='chat',
    content_rowid='id'
);

-- FTS for chat messages
CREATE VIRTUAL TABLE IF NOT EXISTS chat_message_fts USING fts5(
    content,
    content='chat_message',
    content_rowid='id'
);

-- Populate the FTS tables with existing data
INSERT INTO chat_fts(rowid, title)
SELECT id, title FROM chat WHERE title IS NOT NULL;

INSERT INTO chat_message_fts(rowid, content)
SELECT id, content FROM chat_message WHERE content IS NOT NULL;

-- Triggers to keep chat_fts in sync
CREATE TRIGGER IF NOT EXISTS chat_ai AFTER INSERT ON chat BEGIN
  INSERT INTO chat_fts(rowid, title) VALUES (new.id, new.title);
END;

CREATE TRIGGER IF NOT EXISTS chat_ad AFTER DELETE ON chat BEGIN
  DELETE FROM chat_fts WHERE rowid = old.id;
END;

CREATE TRIGGER IF NOT EXISTS chat_au AFTER UPDATE ON chat BEGIN
  UPDATE chat_fts SET title = new.title WHERE rowid = old.id;
END;

-- Triggers to keep chat_message_fts in sync
CREATE TRIGGER IF NOT EXISTS chat_message_ai AFTER INSERT ON chat_message BEGIN
  INSERT INTO chat_message_fts(rowid, content) VALUES (new.id, new.content);
END;

CREATE TRIGGER IF NOT EXISTS chat_message_ad AFTER DELETE ON chat_message BEGIN
  DELETE FROM chat_message_fts WHERE rowid = old.id;
END;

CREATE TRIGGER IF NOT EXISTS chat_message_au AFTER UPDATE ON chat_message BEGIN
  UPDATE chat_message_fts SET content = new.content WHERE rowid = old.id;
END;

