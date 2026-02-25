import { apiGet, apiPost, apiDelete } from './api';

export async function listVehicles() {
  return apiGet('/vehicles');
}

export async function createVehicle(make, model, registration) {
  return apiPost('/vehicles', { make, model, registration });
}

export async function deleteVehicle(vehicleId) {
  return apiDelete(`/vehicles/${vehicleId}`);
}
