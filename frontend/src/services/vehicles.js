import { apiGet, apiPost, apiDelete } from './api';

export async function listVehicles(token) {
  return apiGet('/vehicles', token);
}

export async function createVehicle(make, model, registration, token) {
  return apiPost('/vehicles', { make, model, registration }, token);
}

export async function deleteVehicle(vehicleId, token) {
  return apiDelete(`/vehicles/${vehicleId}`, token);
}
