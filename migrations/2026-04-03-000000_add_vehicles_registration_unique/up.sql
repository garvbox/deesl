-- Add unique index on vehicles(registration, owner_id)
CREATE UNIQUE INDEX idx_vehicles_registration_owner ON vehicles(registration, owner_id);
