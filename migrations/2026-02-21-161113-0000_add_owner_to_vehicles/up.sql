-- Add owner to vehicles
ALTER TABLE vehicles ADD COLUMN owner_id INTEGER NOT NULL REFERENCES users(id) ON DELETE CASCADE;
