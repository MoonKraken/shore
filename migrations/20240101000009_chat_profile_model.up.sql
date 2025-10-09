-- Create chat_profile_model table
CREATE TABLE IF NOT EXISTS chat_profile_model (
    profile_id INTEGER NOT NULL,
    model_id INTEGER NOT NULL,
    display_order INTEGER NOT NULL,
    FOREIGN KEY (model_id) REFERENCES model(id),
    PRIMARY KEY (profile_id, model_id)
);
