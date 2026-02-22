<script setup>
import { ref, onMounted } from 'vue';
import { useAuth } from '../composables/useAuth';
import { listVehicles } from '../services/vehicles';
import { listFuelStations } from '../services/fuelEntries';
import VehicleItem from './VehicleItem.vue';
import AddVehicleForm from './AddVehicleForm.vue';
import FuelEntrySection from './FuelEntrySection.vue';

const { token, userId } = useAuth();

const vehicles = ref([]);
const stations = ref([]);
const loading = ref(true);
const error = ref('');
const showAddForm = ref(false);
const selectedVehicle = ref(null);

async function loadData() {
  try {
    const [v, s] = await Promise.all([
      listVehicles(userId.value, token.value),
      listFuelStations(token.value),
    ]);
    vehicles.value = v;
    stations.value = s;
  } catch (e) {
    error.value = e.message;
  } finally {
    loading.value = false;
  }
}

onMounted(loadData);
</script>

<template>
  <div class="vehicles-page">
    <p v-if="loading">Loading...</p>
    <p v-if="error" class="error">{{ error }}</p>

    <template v-if="!loading">
      <template v-if="!selectedVehicle">
        <div class="section-header">
          <h2>Your Vehicles</h2>
          <button @click="showAddForm = !showAddForm">
            {{ showAddForm ? 'Cancel' : 'Add Vehicle' }}
          </button>
        </div>

        <AddVehicleForm
          v-if="showAddForm"
          @success="showAddForm = false; loadData()"
        />

        <ul v-if="vehicles.length > 0" class="vehicle-list">
          <VehicleItem
            v-for="vehicle in vehicles"
            :key="vehicle.id"
            :vehicle="vehicle"
            @delete="loadData"
            @select="selectedVehicle = $event"
          />
        </ul>

        <p v-else class="empty-state">No vehicles yet. Add your first vehicle above!</p>
      </template>

      <template v-else>
        <button @click="selectedVehicle = null">&lt; Back to Vehicles</button>
        <FuelEntrySection
          :vehicle="selectedVehicle"
          :stations="stations"
          @back="selectedVehicle = null"
        />
      </template>
    </template>
  </div>
</template>
