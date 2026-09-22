CREATE TABLE accounts (
    id TEXT PRIMARY KEY, platform TEXT NOT NULL, puuid TEXT NOT NULL,
    summoner_id TEXT NOT NULL, riot_id TEXT NOT NULL, UNIQUE(platform,puuid)
);
CREATE TABLE games (
    id TEXT PRIMARY KEY, platform TEXT NOT NULL, game_number TEXT NOT NULL,
    detail TEXT, timeline TEXT, data_error TEXT, retry_at INTEGER NOT NULL DEFAULT 0,
    UNIQUE(platform,game_number)
);
CREATE TABLE sessions (
    id TEXT PRIMARY KEY, title TEXT NOT NULL, started_at INTEGER NOT NULL,
    last_activity INTEGER NOT NULL, ended_at INTEGER, end_reason TEXT
);
CREATE UNIQUE INDEX one_open_session ON sessions((1)) WHERE ended_at IS NULL;
CREATE TABLE segments (
    id TEXT PRIMARY KEY, session_id TEXT NOT NULL REFERENCES sessions(id),
    account_id TEXT NOT NULL REFERENCES accounts(id)
);
CREATE TABLE participations (
    id TEXT PRIMARY KEY, game_id TEXT NOT NULL REFERENCES games(id),
    account_id TEXT NOT NULL REFERENCES accounts(id), segment_id TEXT NOT NULL REFERENCES segments(id),
    first_seen INTEGER NOT NULL, last_seen INTEGER NOT NULL, champion_id INTEGER NOT NULL,
    queue_name TEXT NOT NULL, state TEXT NOT NULL CHECK(state IN ('playing','pending','ready')),
    automatic INTEGER NOT NULL CHECK(automatic IN (0,1)), manual INTEGER NOT NULL CHECK(manual IN (0,1)),
    UNIQUE(game_id,account_id)
);
CREATE TABLE user_state (
    id INTEGER PRIMARY KEY CHECK(id=1), data TEXT NOT NULL, revision INTEGER NOT NULL
);
INSERT INTO user_state VALUES(1,'{"notes":[],"reviewed":[],"players":[],"premades":[],"theme":"dark"}',0);
CREATE TABLE review_games (
    game_id TEXT PRIMARY KEY REFERENCES games(id), account_id TEXT NOT NULL REFERENCES accounts(id),
    server TEXT NOT NULL, source TEXT NOT NULL, timeline_error TEXT,
    bookmarked INTEGER NOT NULL DEFAULT 0 CHECK(bookmarked IN (0,1)), viewed_at INTEGER NOT NULL
);
CREATE TABLE replay_files (
    game_id TEXT PRIMARY KEY REFERENCES games(id), bytes INTEGER NOT NULL, version TEXT NOT NULL,
    downloaded_at INTEGER NOT NULL
);
CREATE TABLE subscriptions (
    id TEXT PRIMARY KEY, account_id TEXT NOT NULL UNIQUE REFERENCES accounts(id),
    server TEXT NOT NULL, label TEXT NOT NULL DEFAULT '', paused INTEGER NOT NULL DEFAULT 0 CHECK(paused IN (0,1)),
    generation TEXT NOT NULL, last_synced INTEGER, next_sync INTEGER NOT NULL DEFAULT 0,
    last_error TEXT, limited INTEGER NOT NULL DEFAULT 0 CHECK(limited IN (0,1))
);
CREATE TABLE followed_games (
    subscription_id TEXT NOT NULL REFERENCES subscriptions(id) ON DELETE CASCADE,
    game_id TEXT NOT NULL REFERENCES games(id), PRIMARY KEY(subscription_id, game_id)
);

CREATE TABLE subscription_history (
    subscription_id TEXT PRIMARY KEY REFERENCES subscriptions(id) ON DELETE CASCADE,
    next_start INTEGER NOT NULL DEFAULT 0, anchor TEXT, done INTEGER NOT NULL DEFAULT 0
);
