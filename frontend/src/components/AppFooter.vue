<script setup>
import { ref, onMounted } from 'vue';
import { getVersion } from '../services/version';

const version = ref('');
const error = ref('');

onMounted(async () => {
  try {
    const data = await getVersion();
    version.value = data.version;
  } catch (e) {
    error.value = '';
    // Silently fail - version is not critical
  }
});
</script>

<template>
  <footer class="app-footer">
    <span v-if="version" class="version">v{{ version }}</span>
  </footer>
</template>

<style scoped>
.app-footer {
  padding: 16px 0;
  text-align: center;
  border-top: 1px solid #ddd;
  margin-top: 24px;
}

.version {
  font-size: 0.75rem;
  color: #999;
}
</style>
