<script setup>
import { ref, computed, onMounted } from 'vue';
import { previewImport, executeImport } from '../services/import';
import { listVehicles } from '../services/vehicles';

const emit = defineEmits(['success']);

const vehicles = ref([]);
const step = ref(1);
const selectedVehicleId = ref('');
const file = ref(null);
const previewData = ref(null);
const mappings = ref({});
const loading = ref(false);
const error = ref('');
const importResult = ref(null);

const targetFields = [
  { value: '', label: '-- Skip --' },
  { value: 'filled_at_date', label: 'Date' },
  { value: 'filled_at_time', label: 'Time' },
  { value: 'station', label: 'Station/Location' },
  { value: 'litres', label: 'Litres' },
  { value: 'cost', label: 'Cost' },
  { value: 'mileage_km', label: 'Mileage (KM)' },
];

const canProceedToMapping = computed(() => {
  return selectedVehicleId.value && file.value;
});

const canProceedToPreview = computed(() => {
  const mappedFields = Object.values(mappings.value);
  return mappedFields.includes('filled_at_date') &&
         mappedFields.includes('litres') &&
         mappedFields.includes('cost') &&
         mappedFields.includes('mileage_km');
});

function handleFileChange(event) {
  file.value = event.target.files[0];
}

async function loadPreview() {
  if (!canProceedToMapping.value) return;

  loading.value = true;
  error.value = '';

  try {
    previewData.value = await previewImport(file.value, selectedVehicleId.value);

    mappings.value = { ...previewData.value.suggested_mappings };

    step.value = 2;
  } catch (e) {
    error.value = e.message;
  } finally {
    loading.value = false;
  }
}

function confirmMappings() {
  if (!canProceedToPreview.value) return;
  step.value = 3;
}

async function doImport() {
  loading.value = true;
  error.value = '';

  try {
    importResult.value = await executeImport(
      file.value,
      selectedVehicleId.value,
      mappings.value
    );
    step.value = 4;
    emit('success');
  } catch (e) {
    error.value = e.message;
  } finally {
    loading.value = false;
  }
}

function reset() {
  step.value = 1;
  selectedVehicleId.value = '';
  file.value = null;
  previewData.value = null;
  mappings.value = {};
  importResult.value = null;
  error.value = '';
}

onMounted(async () => {
  try {
    vehicles.value = await listVehicles();
  } catch (e) {
    error.value = 'Failed to load vehicles: ' + e.message;
  }
});
</script>

<template>
  <div class="import-container">
    <h2>Import Fuel Entries</h2>

    <div v-if="error" class="error">{{ error }}</div>

    <div v-if="step === 1" class="step">
      <h3>Step 1: Select Vehicle and CSV File</h3>

      <label>
        <span>Vehicle</span>
        <select v-model="selectedVehicleId">
          <option value="">Select a vehicle...</option>
          <option v-for="vehicle in vehicles" :key="vehicle.id" :value="vehicle.id">
            {{ vehicle.registration }} ({{ vehicle.make }} {{ vehicle.model }})
          </option>
        </select>
      </label>

      <label>
        <span>CSV File</span>
        <input type="file" accept=".csv" @change="handleFileChange" />
      </label>

      <button @click="loadPreview" :disabled="!canProceedToMapping || loading">
        {{ loading ? 'Loading...' : 'Next: Map Fields' }}
      </button>
    </div>

    <div v-if="step === 2" class="step">
      <h3>Step 2: Map CSV Columns</h3>
      <p class="hint">Match each CSV column to the corresponding fuel entry field. Required: Date, Litres, Cost, Mileage.</p>

      <table class="mapping-table">
        <thead>
          <tr>
            <th>CSV Column</th>
            <th>Maps To</th>
            <th>Sample Data</th>
          </tr>
        </thead>
        <tbody>
          <tr v-for="column in previewData.columns" :key="column">
            <td>{{ column }}</td>
            <td>
              <select v-model="mappings[column]">
                <option v-for="field in targetFields" :key="field.value" :value="field.value">
                  {{ field.label }}
                </option>
              </select>
            </td>
            <td class="sample-data">
              {{ previewData.preview[0]?.[previewData.columns.indexOf(column)] || '' }}
            </td>
          </tr>
        </tbody>
      </table>

      <div class="actions">
        <button @click="step = 1">Back</button>
        <button @click="confirmMappings" :disabled="!canProceedToPreview">
          Next: Preview Import
        </button>
      </div>
    </div>

    <div v-if="step === 3" class="step">
      <h3>Step 3: Preview</h3>
      <p class="hint">Review the first few entries before importing.</p>

      <table class="preview-table">
        <thead>
          <tr>
            <th>Date</th>
            <th>Time</th>
            <th>Station</th>
            <th>Litres</th>
            <th>Cost</th>
            <th>Mileage</th>
          </tr>
        </thead>
        <tbody>
          <tr v-for="(row, index) in previewData.preview.slice(0, 5)" :key="index">
            <td>{{ row[previewData.columns.indexOf(mappings.filled_at_date)] || '-' }}</td>
            <td>{{ mappings.filled_at_time ? row[previewData.columns.indexOf(mappings.filled_at_time)] : '-' }}</td>
            <td>{{ mappings.station ? row[previewData.columns.indexOf(mappings.station)] : '-' }}</td>
            <td>{{ row[previewData.columns.indexOf(mappings.litres)] || '-' }}</td>
            <td>{{ row[previewData.columns.indexOf(mappings.cost)] || '-' }}</td>
            <td>{{ row[previewData.columns.indexOf(mappings.mileage_km)] || '-' }}</td>
          </tr>
        </tbody>
      </table>

      <div class="actions">
        <button @click="step = 2">Back</button>
        <button @click="doImport" :disabled="loading" class="primary">
          {{ loading ? 'Importing...' : 'Import Entries' }}
        </button>
      </div>
    </div>

    <div v-if="step === 4" class="step result">
      <h3>Import Complete</h3>

      <div class="result-stats">
        <div class="stat">
          <span class="number">{{ importResult.imported }}</span>
          <span class="label">Imported</span>
        </div>
        <div class="stat">
          <span class="number">{{ importResult.skipped }}</span>
          <span class="label">Skipped (duplicates)</span>
        </div>
        <div class="stat">
          <span class="number">{{ importResult.stations_created }}</span>
          <span class="label">New stations created</span>
        </div>
      </div>

      <div v-if="importResult.errors.length > 0" class="errors">
        <h4>Errors (first 10):</h4>
        <ul>
          <li v-for="(err, index) in importResult.errors" :key="index">{{ err }}</li>
        </ul>
      </div>

      <button @click="reset" class="primary">Import Another File</button>
    </div>
  </div>
</template>

<style scoped>
.import-container {
  background: white;
  padding: 24px;
  border-radius: 8px;
  box-shadow: 0 1px 3px rgba(0,0,0,0.1);
}

.step {
  margin-top: 16px;
}

.step h3 {
  margin-bottom: 16px;
  color: #333;
}

.hint {
  color: #666;
  font-size: 0.875rem;
  margin-bottom: 16px;
}

label {
  display: block;
  margin-bottom: 16px;
}

label span {
  display: block;
  margin-bottom: 4px;
  font-weight: 500;
  color: #555;
}

select, input[type="file"] {
  width: 100%;
  padding: 10px;
  border: 1px solid #ddd;
  border-radius: 4px;
  font-size: 16px;
}

button {
  background: #007bff;
  color: white;
  border: none;
  padding: 12px 24px;
  border-radius: 4px;
  cursor: pointer;
  font-size: 0.875rem;
  margin-right: 8px;
}

button:disabled {
  background: #ccc;
  cursor: not-allowed;
}

button.primary {
  background: #28a745;
}

button.primary:hover:not(:disabled) {
  background: #218838;
}

.mapping-table, .preview-table {
  width: 100%;
  border-collapse: collapse;
  margin-bottom: 16px;
}

.mapping-table th, .mapping-table td,
.preview-table th, .preview-table td {
  padding: 8px 12px;
  text-align: left;
  border-bottom: 1px solid #ddd;
}

.mapping-table th, .preview-table th {
  font-weight: 500;
  color: #555;
  background: #f5f5f5;
}

.sample-data {
  font-size: 0.875rem;
  color: #666;
  max-width: 200px;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.actions {
  margin-top: 16px;
}

.result-stats {
  display: flex;
  gap: 24px;
  margin: 24px 0;
}

.stat {
  text-align: center;
}

.stat .number {
  display: block;
  font-size: 2rem;
  font-weight: bold;
  color: #007bff;
}

.stat .label {
  font-size: 0.875rem;
  color: #666;
}

.errors {
  background: #fff3cd;
  border: 1px solid #ffc107;
  border-radius: 4px;
  padding: 16px;
  margin: 16px 0;
}

.errors h4 {
  margin-bottom: 8px;
  color: #856404;
}

.errors ul {
  margin: 0;
  padding-left: 20px;
}

.errors li {
  color: #856404;
  font-size: 0.875rem;
  margin-bottom: 4px;
}
</style>
