CREATE TABLE game_events (
    id BIGSERIAL PRIMARY KEY,
    game_id UUID NOT NULL REFERENCES games(id) ON DELETE CASCADE,
    seq_num INTEGER NOT NULL,
    event_type VARCHAR(32) NOT NULL,
    actor_id UUID,
    data JSONB NOT NULL DEFAULT '{}',
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_game_events_game_id ON game_events(game_id, seq_num);
