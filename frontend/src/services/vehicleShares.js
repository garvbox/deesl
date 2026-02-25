import { apiGet, apiPost, apiDelete } from './api';

export async function listVehicleShares() {
  return apiGet('/vehicle-shares');
}

export async function shareVehicle(vehicleId, sharedWithEmail, permissionLevel) {
  return apiPost('/vehicle-shares', { 
    vehicle_id: vehicleId, 
    shared_with_email: sharedWithEmail, 
    permission_level: permissionLevel 
  });
}

export async function unshareVehicle(shareId) {
  return apiDelete(`/vehicle-shares/${shareId}`);
}
