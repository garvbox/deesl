<script setup>
import { ref, onMounted } from 'vue';
import { useAuth } from '../composables/useAuth';
import { listVehicles } from '../services/vehicles';
import { listFuelStations, listRecentFuelEntries } from '../services/fuelEntries';
import VehicleItem from './VehicleItem.vue';
import AddVehicleForm from './AddVehicleForm.vue';
import FuelEntrySection from './FuelEntrySection.vue';
import FuelEntryItem from './FuelEntryItem.vue';
import QuickAddFuelForm from './QuickAddFuelForm.vue';

const { token, userId } = useAuth();

const vehicles = ref([]);
const stations = ref([]);
const recentEntries = ref([]);
const loading = ref(true);
const error = ref('');
const showAddForm = ref(false);
const selectedVehicle = ref(null);
const showQuickAdd = ref(false);

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

async function loadRecentEntries() {
  if (!userId.value) return;
  try {
    recentEntries.value = await listRecentFuelEntries(userId.value, token.value);
  } catch (e) {
    console.error('Failed to load recent entries:', e);
  }
}

onMounted(() => {
  loadData();
  loadRecentEntries();
});
</script>

<template>
  <div class="dashboard">
    <p v-if="loading">Loading...</p>
    <p class="error">{{ error }}</p>

    <template v-if="!loading">
      <template v-if="!selectedVehicle">
        <section class="recent-entries-section">
          <div class="section-header">
            <h2>Recent Fuel Entries</h2>
            <button @click="showQuickAdd = !showQuickAdd">
              {{ showQuickAdd ? 'Cancel' : 'Quick Add' }}
            </button>
          </div>

          <QuickAddFuelForm
            v-if="showQuickAdd"
            :vehicles="vehicles"
            :stations="stations"
            @success="showQuickAdd = false; loadRecentEntries()"
          />

          <ul v-if="recentEntries.length > 0" class="entry-list">
            <FuelEntryItem
              v-for="entry in recentEntries"
              :key="entry.id"
              :entry="entry"
              :show-vehicle="true"
              @delete="loadRecentEntries"
            />
          </ul>

          <p v-else class="empty-state">No fuel entries yet. Add your first entry!</p>
        </section>

        <section class="vehicles-section">
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
        </section>
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
