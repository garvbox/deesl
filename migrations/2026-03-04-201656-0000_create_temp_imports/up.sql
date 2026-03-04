CREATE TABLE temp_imports (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    user_id INTEGER NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    vehicle_id INTEGER NOT NULL REFERENCES vehicles(id) ON DELETE CASCADE,
    csv_data BYTEA NOT NULL,
    created_at TIMESTAMP NOT NULL DEFAULT NOW()
);

-- Create index for cleanup of old imports
CREATE INDEX idx_temp_imports_created_at ON temp_imports(created_at);
CREATE INDEX idx_temp_imports_user_id ON temp_imports(user_id);
