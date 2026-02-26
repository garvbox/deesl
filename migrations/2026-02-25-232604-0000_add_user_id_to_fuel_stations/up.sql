-- Add user_id column to fuel_stations table
ALTER TABLE fuel_stations
ADD COLUMN user_id INTEGER REFERENCES users(id);

-- Create index for faster lookups
CREATE INDEX idx_fuel_stations_user_id ON fuel_stations(user_id);
