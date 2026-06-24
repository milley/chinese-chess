-- Remove the redundant move_history column.
-- Move data is now stored in the game_events table (event_type = 'move')
-- and can be reconstructed via game_repo.get_move_history_from_events().
ALTER TABLE games DROP COLUMN IF EXISTS move_history;
