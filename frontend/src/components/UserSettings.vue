<script setup>
import { ref } from 'vue';
import { useAuth } from '../composables/useAuth';
import { useCurrency } from '../composables/useCurrency';

const { userId, token } = useAuth();
const { currency, supportedCurrencies, saveCurrency } = useCurrency();

const saving = ref(false);
const error = ref(null);

async function handleCurrencyChange(event) {
  saving.value = true;
  error.value = null;
  try {
    await saveCurrency(event.target.value, userId.value, token.value);
  } catch (e) {
    error.value = e.message;
  } finally {
    saving.value = false;
  }
}
</script>

<template>
  <div class="user-settings">
    <label>
      <span>Currency</span>
      <select :value="currency" @change="handleCurrencyChange" :disabled="saving">
        <option v-for="c in supportedCurrencies" :key="c" :value="c">{{ c }}</option>
      </select>
    </label>
    <span v-if="error" class="error">{{ error }}</span>
  </div>
</template>

<style scoped>
.user-settings {
  display: flex;
  align-items: center;
  gap: 8px;
}

.user-settings label {
  display: flex;
  align-items: center;
  gap: 6px;
  margin: 0;
  font-size: 0.875rem;
}

.user-settings label span {
  color: #555;
  font-weight: 500;
  white-space: nowrap;
}

.user-settings select {
  width: auto;
  padding: 4px 8px;
  font-size: 0.875rem;
}
</style>
