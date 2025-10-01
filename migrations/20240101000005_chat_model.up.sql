-- Create chat_model table
CREATE TABLE IF NOT EXISTS chat_model (
    chat_id INTEGER NOT NULL,
    model_id INTEGER NOT NULL,
    FOREIGN KEY (chat_id) REFERENCES chat(id) ON DELETE CASCADE,
    FOREIGN KEY (model_id) REFERENCES model(id),
    PRIMARY KEY (chat_id, model_id)
);
