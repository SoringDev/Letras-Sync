CREATE TABLE music (
    id TEXT PRIMARY KEY,
    title TEXT NOT NULL,
    artist TEXT,
    youtube_url TEXT NOT NULL,
    duration INTEGER,
    thumbnail TEXT,
    created_at DATETIME DEFAULT CURRENT_TIMESTAMP
);

CREATE TABLE lyrics_line (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    music_id TEXT NOT NULL,
    start_time REAL NOT NULL,
    end_time REAL NOT NULL,
    text TEXT NOT NULL,
    FOREIGN KEY (music_id) REFERENCES music(id)
);
