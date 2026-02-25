import { apiGet, apiPost, apiDelete } from './api';

export async function listFuelEntries(vehicleId, token) {
  return apiGet(`/fuel-entries?vehicle_id=${vehicleId}`, token);
}

export async function listRecentFuelEntries(token, limit = 10) {
  return apiGet(`/fuel-entries?limit=${limit}`, token);
}

export async function createFuelEntry(vehicleId, stationId, mileage, litres, cost, token, filledAt = null) {
  return apiPost('/fuel-entries', {
    vehicle_id: vehicleId,
    station_id: stationId,
    mileage_km: mileage,
    litres,
    cost,
    filled_at: filledAt,
  }, token);
}

export async function deleteFuelEntry(entryId, token) {
  return apiDelete(`/fuel-entries/${entryId}`, token);
}

export async function listFuelStations(token) {
  return apiGet('/fuel-stations', token);
}
