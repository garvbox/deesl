-- Add index on fuel_entries(filled_at) for faster stats queries
CREATE INDEX idx_fuel_entries_filled_at ON fuel_entries(filled_at);
