<script setup>
import { ref } from 'vue';
import { deleteFuelEntry } from '../services/fuelEntries';
import { useAuth } from '../composables/useAuth';

const props = defineProps({
  entry: Object,
  showVehicle: { type: Boolean, default: false },
});

const emit = defineEmits(['delete']);

const { token } = useAuth();
const showConfirm = ref(false);
const deleting = ref(false);

async function handleDelete() {
  deleting.value = true;
  try {
    await deleteFuelEntry(props.entry.id, token.value);
    emit('delete');
  } catch (e) {
    console.error(e);
  }
}
</script>

<template>
  <li class="entry-item">
    <div class="entry-info">
      <span v-if="showVehicle" class="vehicle">{{ entry.vehicle_make }} {{ entry.vehicle_model }}</span>
      <span class="date">{{ entry.filled_at }}</span>
      <span class="mileage">{{ entry.mileage_km }} km</span>
      <span class="litres">{{ entry.litres }} L</span>
      <span class="cost">${{ entry.cost }}</span>
      <span v-if="entry.station_name" class="station">{{ entry.station_name }}</span>
    </div>
    <div class="entry-actions">
      <template v-if="showConfirm">
        <span>Confirm? </span>
        <button @click="handleDelete" :disabled="deleting">
          {{ deleting ? 'Deleting...' : 'Yes' }}
        </button>
        <button @click="showConfirm = false" :disabled="deleting">No</button>
      </template>
      <button v-else class="delete-btn" @click="showConfirm = true">Delete</button>
    </div>
  </li>
</template>
