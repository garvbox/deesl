import { apiGet, apiPost, apiDelete } from './api';

export async function listVehicleShares(token) {
  return apiGet('/vehicle-shares', token);
}

export async function shareVehicle(vehicleId, sharedWithEmail, permissionLevel, token) {
  return apiPost('/vehicle-shares', { 
    vehicle_id: vehicleId, 
    shared_with_email: sharedWithEmail, 
    permission_level: permissionLevel 
  }, token);
}

export async function unshareVehicle(shareId, token) {
  return apiDelete(`/vehicle-shares/${shareId}`, token);
}
