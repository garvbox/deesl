<script setup>
import { ref, computed } from 'vue';
import { createFuelEntry } from '../services/fuelEntries';

const props = defineProps({
  vehicles: Array,
  stations: Array,
  defaultVehicleId: Number,
});

const emit = defineEmits(['success']);

const selectedVehicleId = ref(props.defaultVehicleId ? props.defaultVehicleId.toString() : '');
const stationQuery = ref('');
const selectedStationId = ref(null);
const mileage = ref('');
const litres = ref('');
const cost = ref('');
const filledAt = ref(new Date().toISOString().slice(0, 16));
const error = ref('');
const loading = ref(false);
const showDropdown = ref(false);

const filteredStations = computed(() => {
  const query = stationQuery.value.toLowerCase();
  return props.stations
    .filter(s => s.name.toLowerCase().includes(query))
    .slice(0, 5);
});

function selectStation(station) {
  stationQuery.value = station.name;
  selectedStationId.value = station.id;
  showDropdown.value = false;
}

async function handleSubmit() {
  if (!selectedVehicleId.value) {
    error.value = 'Please select a vehicle';
    return;
  }

  const mileageVal = parseInt(mileage.value, 10);
  const litresVal = parseFloat(litres.value);
  const costVal = parseFloat(cost.value);

  if (isNaN(mileageVal) || isNaN(litresVal) || isNaN(costVal) || litresVal <= 0 || costVal <= 0) {
    error.value = 'Please enter valid numbers';
    return;
  }

  loading.value = true;
  error.value = '';

  try {
    await createFuelEntry(
      parseInt(selectedVehicleId.value, 10),
      selectedStationId.value,
      mileageVal,
      litresVal,
      costVal,
      filledAt.value ? new Date(filledAt.value).toISOString() : null
    );
    selectedVehicleId.value = '';
    mileage.value = '';
    litres.value = '';
    cost.value = '';
    stationQuery.value = '';
    selectedStationId.value = null;
    filledAt.value = new Date().toISOString().slice(0, 16);
    emit('success');
  } catch (e) {
    error.value = e.message;
  } finally {
    loading.value = false;
  }
}

function hideDropdown() {
  setTimeout(() => { showDropdown.value = false; }, 200);
}
</script>

<template>
  <form class="add-entry-form" @submit.prevent="handleSubmit">
    <label>
      <span>Vehicle</span>
      <select v-model="selectedVehicleId" :disabled="loading" required>
        <option value="">Select a vehicle</option>
        <option v-for="v in vehicles" :key="v.id" :value="v.id">
          {{ v.make }} {{ v.model }} ({{ v.registration }})
        </option>
      </select>
    </label>
    <label>
      <span>Station (optional)</span>
      <input
        type="text"
        v-model="stationQuery"
        placeholder="Type to search or add new"
        @focus="showDropdown = true"
        @blur="hideDropdown"
        :disabled="loading"
      />
      <ul v-if="showDropdown && filteredStations.length > 0" class="autocomplete-dropdown">
        <li
          v-for="station in filteredStations"
          :key="station.id"
          @click="selectStation(station)"
        >
          {{ station.name }}
        </li>
      </ul>
    </label>
    <label>
      <span>Mileage (km)</span>
      <input type="number" v-model="mileage" placeholder="Current odometer reading" :disabled="loading" />
    </label>
    <label>
      <span>Litres</span>
      <input type="number" step="0.01" v-model="litres" placeholder="Amount filled" :disabled="loading" />
    </label>
    <label>
      <span>Cost</span>
      <input type="number" step="0.01" v-model="cost" placeholder="Total cost" :disabled="loading" />
    </label>
    <label>
      <span>Date/Time (optional)</span>
      <input type="datetime-local" v-model="filledAt" :disabled="loading" />
    </label>
    <button type="submit" :disabled="loading">
      {{ loading ? 'Saving...' : 'Add Entry' }}
    </button>
    <p class="error">{{ error }}</p>
  </form>
</template>
