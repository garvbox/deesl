<script setup>
import { ref } from 'vue';
import { deleteVehicle } from '../services/vehicles';
import { useAuth } from '../composables/useAuth';

const props = defineProps({
  vehicle: Object,
});

const emit = defineEmits(['delete', 'select']);

const { token } = useAuth();
const showConfirm = ref(false);
const deleting = ref(false);

async function handleDelete() {
  deleting.value = true;
  try {
    await deleteVehicle(props.vehicle.id, token.value);
    emit('delete');
  } catch (e) {
    console.error(e);
  }
}
</script>

<template>
  <li class="vehicle-item">
    <div class="vehicle-info" @click="emit('select', vehicle)">
      <strong>{{ vehicle.make }} {{ vehicle.model }}</strong>
      <span class="registration">{{ vehicle.registration }}</span>
    </div>
    <div class="vehicle-actions">
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
