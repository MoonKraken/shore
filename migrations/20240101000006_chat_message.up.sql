-- Create chat_message table
CREATE TABLE IF NOT EXISTS chat_message (
    id INTEGER NOT NULL PRIMARY KEY AUTOINCREMENT,
    chat_id INTEGER NOT NULL,
    dt INTEGER NOT NULL,
    chat_role TEXT NOT NULL,
    content TEXT,
    reasoning_content TEXT,
    tool_calls TEXT,
    tool_call_id TEXT,
    name TEXT,
    model_id INTEGER, -- this is null for user tool result messages
    error TEXT,
    FOREIGN KEY (chat_id) REFERENCES chat(id) ON DELETE CASCADE
    FOREIGN KEY (model_id) REFERENCES model(id)
);
