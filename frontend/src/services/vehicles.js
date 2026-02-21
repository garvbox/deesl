import { apiGet, apiPost, apiDelete } from './api';

export async function listVehicles(userId, token) {
  return apiGet(`/vehicles?user_id=${userId}`, token);
}

export async function createVehicle(make, model, registration, userId, token) {
  return apiPost('/vehicles', { make, model, registration, owner_id: userId }, token);
}

export async function deleteVehicle(vehicleId, token) {
  return apiDelete(`/vehicles/${vehicleId}`, token);
}
