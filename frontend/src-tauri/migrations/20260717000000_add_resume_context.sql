-- Single active "resume context" fed into summary generation as background context.
-- Singleton row (id = 1); replaced on each upload.
CREATE TABLE IF NOT EXISTS resume_context (
    id INTEGER PRIMARY KEY CHECK (id = 1),
    content TEXT NOT NULL,
    filename TEXT,
    updated_at TEXT NOT NULL
);
