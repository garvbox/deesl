<script setup>
import { ref, onMounted } from 'vue';
import { listFuelEntries } from '../services/fuelEntries';
import AddFuelEntryForm from './AddFuelEntryForm.vue';
import FuelEntryItem from './FuelEntryItem.vue';

const props = defineProps({
  vehicle: Object,
  stations: Array,
});

const emit = defineEmits(['back']);

const entries = ref([]);
const loading = ref(true);
const error = ref('');
const showAddForm = ref(false);

async function loadEntries() {
  try {
    entries.value = await listFuelEntries(props.vehicle.id);
  } catch (e) {
    error.value = e.message;
  } finally {
    loading.value = false;
  }
}

onMounted(loadEntries);
</script>

<template>
  <section class="fuel-entry-section">
    <h2>{{ vehicle.make }} {{ vehicle.model }} ({{ vehicle.registration }})</h2>

    <div class="section-header">
      <h3>Fuel Entries</h3>
      <button @click="showAddForm = !showAddForm">
        {{ showAddForm ? 'Cancel' : 'Add Entry' }}
      </button>
    </div>

    <AddFuelEntryForm
      v-if="showAddForm"
      :vehicle-id="vehicle.id"
      :stations="stations"
      @success="showAddForm = false; loadEntries()"
    />

    <p v-if="loading">Loading entries...</p>
    <p class="error">{{ error }}</p>

    <ul v-if="!loading && entries.length > 0" class="entry-list">
      <FuelEntryItem
        v-for="entry in entries"
        :key="entry.id"
        :entry="entry"
        @delete="loadEntries"
      />
    </ul>

    <p v-if="!loading && entries.length === 0" class="empty-state">
      No fuel entries yet. Add your first entry above!
    </p>
  </section>
</template>
