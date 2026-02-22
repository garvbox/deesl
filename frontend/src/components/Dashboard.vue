<script setup>
import { ref, onMounted } from 'vue';
import { useAuth } from '../composables/useAuth';
import { listVehicles } from '../services/vehicles';
import { listFuelStations, listRecentFuelEntries } from '../services/fuelEntries';
import FuelEntryItem from './FuelEntryItem.vue';
import QuickAddFuelForm from './QuickAddFuelForm.vue';

const { token, userId } = useAuth();

const vehicles = ref([]);
const stations = ref([]);
const recentEntries = ref([]);
const loading = ref(true);
const error = ref('');
const showQuickAdd = ref(false);

async function loadData() {
  try {
    const [v, s, entries] = await Promise.all([
      listVehicles(userId.value, token.value),
      listFuelStations(token.value),
      listRecentFuelEntries(userId.value, token.value),
    ]);
    vehicles.value = v;
    stations.value = s;
    recentEntries.value = entries;
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

onMounted(loadData);
</script>

<template>
  <div class="dashboard">
    <p v-if="loading">Loading...</p>
    <p v-if="error" class="error">{{ error }}</p>

    <template v-if="!loading">
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
    </template>
  </div>
</template>
