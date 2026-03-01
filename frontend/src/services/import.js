const API_BASE = '/api';

// Convert mappings from frontend format {csvColumn: targetField} to backend format {targetField: csvColumn}
function invertMappings(mappings) {
  const inverted = {};
  for (const [csvColumn, targetField] of Object.entries(mappings)) {
    if (targetField && targetField !== '') {
      inverted[targetField] = csvColumn;
    }
  }
  return inverted;
}

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
  // Invert mappings to match backend expected format
  formData.append('mappings', JSON.stringify(invertMappings(mappings)));

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
