-- Create chat_tool table
CREATE TABLE IF NOT EXISTS chat_tool (
    chat_id INTEGER NOT NULL,
    tool_id INTEGER NOT NULL,
    FOREIGN KEY (chat_id) REFERENCES chat(id) ON DELETE CASCADE,
    FOREIGN KEY (tool_id) REFERENCES tool(id),
    PRIMARY KEY (chat_id, tool_id)
);
