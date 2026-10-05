# TODO

Suspicious behaviours found while characterising the API/route surface with integration
tests. These are pinned by tests (where practical) and should be reviewed during the
upcoming refactor.

## Auth / OAuth

- `is_dev_auth_bypass_allowed` checks only the `DEV_AUTH_EMAIL` env var and ignores the
  request headers/host, so when the `dev` feature is built with the var set, any request
  is authenticated as user id 1. See `src/auth.rs:128-137`.

## Vehicles

- `DELETE /vehicles/htmx/{id}` returns `200` with an empty body for both non-owned and
  non-existent vehicles; no `403`/`404`. See `src/handlers/vehicles.rs:113-131`.
- `GET /vehicles/htmx/list` only lists owned vehicles, not vehicles shared with the user,
  even though shared users can create fuel entries. Inconsistent visibility.
  See `src/handlers/vehicles.rs:98-105`.

## Fuel entries

- `POST /fuel-entries` takes `station_id: i32` (required) and never verifies the station
  belongs to, or is visible to, the user. See `src/handlers/fuel_entries.rs:79-114`.
- An unparseable `filled_at` is silently dropped (column default `now()` is used) instead
  of returning `400`. See `src/handlers/fuel_entries.rs:98-100`.
- `GET /fuel-entries/htmx/recent` and the list page filter by `vehicles.owner_id`, so
  shared vehicles never appear. See `src/handlers/fuel_entries.rs:178` and `:390`.

## Stations

- `update_station` / `delete_station` require `fuel_stations.user_id = user_id`, so global
  (`user_id IS NULL`) stations are treated as forbidden. `merge_stations` does allow
  global stations as the target. Inconsistent. See `src/handlers/stations.rs:99-113`.
- `merge_stations` reassigns *all* fuel entries referencing the source station, with no
  ownership filter on the entries, so entries on other users' vehicles can be mutated.
  See `src/handlers/stations.rs:173-178`.

## Import

- `parse_float` strips `,` but not spaces; `parse_int` strips both. Inconsistent handling
  of thousands separators. See `src/handlers/import.rs:642-648`.

## Stats

- Dead code: an unused `entries` boxed query is constructed and never executed.
  See `src/handlers/stats.rs:138-143`.
- Cost axis labels are hardcoded to `€` regardless of the user's currency preference.
  See `src/handlers/stats.rs:319-329`.
- Shared vehicles are excluded (filters on `vehicles.owner_id`). See `src/handlers/stats.rs:142`.

## Error mapping

- `check_vehicle_write_access` maps a missing vehicle to `DieselError::NotFound`, which
  becomes `AppError::Database(NotFound)` -> `500`, rather than a `404`. See
  `src/handlers/mod.rs:42-46` and `src/error.rs:98`.
- `merge_stations` returns `diesel::result::Error::NotFound` from inside its transaction
  for a missing/non-owned source or target, which also surfaces as `500` rather than
  `404`. See `src/handlers/stations.rs:147-186`.
- More generally, `DieselError::NotFound` is mapped to a `500` in `From<DieselError>`
  rather than `404`, so any handler that propagates it via `?` leaks a server error.
  See `src/error.rs:95-114`.
