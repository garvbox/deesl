<script setup>
import { ref } from 'vue';
import { createVehicle } from '../services/vehicles';
import { useAuth } from '../composables/useAuth';

const emit = defineEmits(['success']);

const { token } = useAuth();

const make = ref('');
const model = ref('');
const registration = ref('');
const error = ref('');
const loading = ref(false);

async function handleSubmit() {
  if (!make.value || !model.value || !registration.value) {
    error.value = 'Please fill in all fields';
    return;
  }

  loading.value = true;
  error.value = '';

  try {
    await createVehicle(make.value, model.value, registration.value, token.value);
    make.value = '';
    model.value = '';
    registration.value = '';
    emit('success');
  } catch (e) {
    error.value = e.message;
  } finally {
    loading.value = false;
  }
}
</script>

<template>
  <form class="add-vehicle-form" @submit.prevent="handleSubmit">
    <label>
      <span>Make</span>
      <input type="text" v-model="make" placeholder="e.g., Toyota" :disabled="loading" />
    </label>
    <label>
      <span>Model</span>
      <input type="text" v-model="model" placeholder="e.g., Corolla" :disabled="loading" />
    </label>
    <label>
      <span>Registration</span>
      <input type="text" v-model="registration" placeholder="e.g., ABC123" :disabled="loading" />
    </label>
    <button type="submit" :disabled="loading">
      {{ loading ? 'Adding...' : 'Add Vehicle' }}
    </button>
    <p class="error">{{ error }}</p>
  </form>
</template>
