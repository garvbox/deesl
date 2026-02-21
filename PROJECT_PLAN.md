# Deesl Fuel Tracker - Project Plan

## Overview
A Rust-based fuel tracking application with a Leptos SPA frontend and Axum/Diesel backend using PostgreSQL.

## Technology Stack
- **Backend**: Axum + Diesel + PostgreSQL
- **Frontend**: Leptos (WASM SPA)
- **Authentication**: JWT tokens
- **API**: REST with OpenAPI 3.0 spec

## Features

### Phase 1: User Management (Completed)
- User registration and login
- JWT-based authentication
- Per-user vehicle ownership

### Phase 2: Vehicle Management (Completed)
- Add/view/delete vehicles
- Vehicles owned by specific users

### Phase 3: Fuel Tracking (Completed)
- Quick fuel entry form:
  - Fuel station (with autocomplete from history)
  - Current mileage (in km)
  - Amount topped up (in litres)
  - Cost of top-up
- Remember fuel stations for quick entry

### Phase 4: Vehicle Sharing (Future)
- Share vehicles with other users
- Different permission levels (read/write)

---

## Implementation Plan

### Step 1: Database Schema ✅
- [x] users table
- [x] vehicles table (with owner_id)
- [x] fuel_stations table
- [x] fuel_entries table
- [x] vehicle_shares table (future)

### Step 2: Backend Models ✅
- [x] User, Vehicle, FuelStation, FuelEntry, VehicleShare models
- [x] Use f64 for litres/cost (PostgreSQL DOUBLE PRECISION)

### Step 3: Authentication API ✅
- [x] POST /api/auth/register
- [x] POST /api/auth/login
- [x] JWT token generation and validation

### Step 4: OpenAPI Documentation ✅
- [x] /api/openapi.json endpoint
- [x] Schema definitions for requests/responses

### Step 5: CRUD API Endpoints ✅
- [x] GET/POST /api/fuel-stations
- [x] DELETE /api/fuel-stations/{id}
- [x] GET/POST /api/fuel-entries
- [x] DELETE /api/fuel-entries/{id}
- [x] GET/POST /api/vehicles
- [x] GET /api/vehicles?user_id=

### Step 6: Frontend Setup ✅
- [x] Leptos workspace setup
- [x] WASM compilation
- [x] Static file serving
- [x] Basic SPA structure

### Step 7: Frontend Authentication ✅
- [x] Login form component
- [x] Register form component  
- [x] API integration with /api/auth/*
- [x] JWT token storage (localStorage)
- [x] Auth state management
- [x] Protected routes

### Step 8: Vehicle Management UI ✅
- [x] Vehicle list view
- [x] Add vehicle form
- [x] Delete vehicle functionality
- [x] Filter by current user

### Step 9: Fuel Entry UI ✅
- [x] Quick fuel entry form
- [x] Station autocomplete from history
- [x] Fuel entry list per vehicle
- [x] Mobile-optimised input fields

### Step 10: Testing & Polish ✅
- [x] API testing with OpenAPI spec
- [x] Mobile responsiveness
- [x] Error handling
- [x] Build verification

---

## API Endpoints

### Authentication
| Method | Path | Description |
|--------|------|-------------|
| POST | /api/auth/register | Register new user |
| POST | /api/auth/login | Login, returns JWT |

### Vehicles
| Method | Path | Description |
|--------|------|-------------|
| GET | /api/vehicles?user_id= | List vehicles (filtered by user) |
| POST | /api/vehicles | Create vehicle |

### Fuel Stations
| Method | Path | Description |
|--------|------|-------------|
| GET | /api/fuel-stations | List all stations |
| POST | /api/fuel-stations | Create station |
| DELETE | /api/fuel-stations/{id} | Delete station |

### Fuel Entries
| Method | Path | Description |
|--------|------|-------------|
| GET | /api/fuel-entries?vehicle_id= | List entries (filtered by vehicle) |
| POST | /api/fuel-entries | Create entry |
| GET | /api/fuel-entries/{id} | Get single entry |
| DELETE | /api/fuel-entries/{id} | Delete entry |

### Documentation
| Method | Path | Description |
|--------|------|-------------|
| GET | /api/openapi.json | OpenAPI 3.0 spec |
| GET | / | Frontend SPA |

---

## Database Schema

```sql
-- Users
CREATE TABLE users (
  id SERIAL PRIMARY KEY,
  email TEXT NOT NULL UNIQUE,
  password_hash TEXT NOT NULL,
  created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP
);

-- Vehicles (owned by users)
CREATE TABLE vehicles (
  id SERIAL PRIMARY KEY,
  make TEXT NOT NULL,
  model TEXT NOT NULL,
  registration TEXT NOT NULL,
  created TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
  owner_id INTEGER NOT NULL REFERENCES users(id) ON DELETE CASCADE
);

-- Fuel Stations
CREATE TABLE fuel_stations (
  id SERIAL PRIMARY KEY,
  name TEXT NOT NULL,
  created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP
);

-- Fuel Entries
CREATE TABLE fuel_entries (
  id SERIAL PRIMARY KEY,
  vehicle_id INTEGER NOT NULL REFERENCES vehicles(id) ON DELETE CASCADE,
  station_id INTEGER REFERENCES fuel_stations(id) ON DELETE SET NULL,
  mileage_km INTEGER NOT NULL,
  litres DOUBLE PRECISION NOT NULL,
  cost DOUBLE PRECISION NOT NULL,
  filled_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
  created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP
);

-- Vehicle Sharing (future)
CREATE TABLE vehicle_shares (
  id SERIAL PRIMARY KEY,
  vehicle_id INTEGER NOT NULL REFERENCES vehicles(id) ON DELETE CASCADE,
  shared_with_user_id INTEGER NOT NULL REFERENCES users(id) ON DELETE CASCADE,
  permission_level TEXT NOT NULL DEFAULT 'read',
  created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
  UNIQUE(vehicle_id, shared_with_user_id)
);
```

---

## Running the Project

### Development
```bash
# Start database
docker compose up -d

# Run migrations
diesel migration run

# Start server
cargo run
```

### Frontend Development
```bash
# Build frontend
cd frontend
wasm-pack build --target web --out-dir ../src/pkg
```

### Testing
```bash
cargo test
```
