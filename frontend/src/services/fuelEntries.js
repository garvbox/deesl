import { apiGet, apiPost, apiDelete } from './api';

export async function listFuelEntries(vehicleId) {
  const query = vehicleId ? `?vehicle_id=${vehicleId}` : '';
  return apiGet(`/fuel-entries${query}`);
}

export async function listRecentFuelEntries(limit = 10) {
  return apiGet(`/fuel-entries?limit=${limit}`);
}

export async function createFuelEntry(vehicleId, stationId, mileage, litres, cost, filledAt = null) {
  return apiPost('/fuel-entries', {
    vehicle_id: vehicleId,
    station_id: stationId,
    mileage_km: mileage,
    litres,
    cost,
    filled_at: filledAt,
  });
}

export async function deleteFuelEntry(entryId) {
  return apiDelete(`/fuel-entries/${entryId}`);
}

export async function listFuelStations() {
  return apiGet('/fuel-stations');
}
