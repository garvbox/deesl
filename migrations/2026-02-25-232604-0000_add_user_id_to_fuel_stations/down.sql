-- Remove user_id column from fuel_stations table
DROP INDEX IF EXISTS idx_fuel_stations_user_id;
ALTER TABLE fuel_stations DROP COLUMN IF EXISTS user_id;
