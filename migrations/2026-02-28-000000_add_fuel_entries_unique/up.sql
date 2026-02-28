CREATE UNIQUE INDEX idx_fuel_entries_vehicle_mileage_datetime 
ON fuel_entries (vehicle_id, mileage_km, filled_at);
