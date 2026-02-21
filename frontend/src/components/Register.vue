<script setup>
import { ref } from 'vue';
import { useAuth } from '../composables/useAuth';

const emit = defineEmits(['switch-to-login']);

const { register } = useAuth();

const email = ref('');
const password = ref('');
const confirmPassword = ref('');
const error = ref('');
const loading = ref(false);

async function handleSubmit() {
  if (!email.value || !password.value) {
    error.value = 'Please fill in all fields';
    return;
  }

  if (password.value !== confirmPassword.value) {
    error.value = 'Passwords do not match';
    return;
  }

  if (password.value.length < 6) {
    error.value = 'Password must be at least 6 characters';
    return;
  }

  loading.value = true;
  error.value = '';

  try {
    await register(email.value, password.value);
  } catch (e) {
    error.value = e.message;
    loading.value = false;
  }
}
</script>

<template>
  <div class="register">
    <h2>Register</h2>
    <form @submit.prevent="handleSubmit">
      <label>
        <span>Email</span>
        <input type="email" v-model="email" :disabled="loading" />
      </label>
      <label>
        <span>Password</span>
        <input type="password" v-model="password" :disabled="loading" />
      </label>
      <label>
        <span>Confirm Password</span>
        <input type="password" v-model="confirmPassword" :disabled="loading" />
      </label>
      <button type="submit" :disabled="loading">
        {{ loading ? 'Creating account...' : 'Register' }}
      </button>
    </form>
    <p class="error">{{ error }}</p>
    <p class="switch-form">
      Already have an account?
      <a @click="emit('switch-to-login')">Login</a>
    </p>
  </div>
</template>
