const API_BASE = '/api';

export async function previewImport(file, vehicleId) {
  const formData = new FormData();
  formData.append('file', file);
  formData.append('vehicle_id', vehicleId);

  const response = await fetch(`${API_BASE}/fuel-entries/import/preview`, {
    method: 'POST',
    credentials: 'include',
    body: formData,
  });

  if (!response.ok) {
    const text = await response.text();
    throw new Error(text || `HTTP ${response.status}`);
  }

  return response.json();
}

export async function executeImport(file, vehicleId, mappings) {
  const formData = new FormData();
  formData.append('file', file);
  formData.append('vehicle_id', vehicleId);
  formData.append('mappings', JSON.stringify(mappings));

  const response = await fetch(`${API_BASE}/fuel-entries/import`, {
    method: 'POST',
    credentials: 'include',
    body: formData,
  });

  if (!response.ok) {
    const text = await response.text();
    throw new Error(text || `HTTP ${response.status}`);
  }

  return response.json();
}
