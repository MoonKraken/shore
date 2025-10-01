-- Create chat_profile_tool table
CREATE TABLE IF NOT EXISTS chat_profile_tool (
    profile_id INTEGER NOT NULL,
    tool_id INTEGER NOT NULL,
    FOREIGN KEY (tool_id) REFERENCES tool(id),
    PRIMARY KEY (profile_id, tool_id)
);
