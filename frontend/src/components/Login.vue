<script setup>
import { ref } from 'vue';
import { useAuth } from '../composables/useAuth';

const emit = defineEmits(['switch-to-register']);

const { login } = useAuth();

const email = ref('');
const password = ref('');
const error = ref('');
const loading = ref(false);

async function handleSubmit() {
  if (!email.value || !password.value) {
    error.value = 'Please fill in all fields';
    return;
  }

  loading.value = true;
  error.value = '';

  try {
    await login(email.value, password.value);
  } catch (e) {
    error.value = e.message;
    loading.value = false;
  }
}
</script>

<template>
  <div class="login">
    <h2>Login</h2>
    <form @submit.prevent="handleSubmit">
      <label>
        <span>Email</span>
        <input type="email" v-model="email" :disabled="loading" />
      </label>
      <label>
        <span>Password</span>
        <input type="password" v-model="password" :disabled="loading" />
      </label>
      <button type="submit" :disabled="loading">
        {{ loading ? 'Logging in...' : 'Login' }}
      </button>
    </form>
    <p class="error">{{ error }}</p>
    <p class="switch-form">
      Don't have an account?
      <a @click="emit('switch-to-register')">Register</a>
    </p>
  </div>
</template>
