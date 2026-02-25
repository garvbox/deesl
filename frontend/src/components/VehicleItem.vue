<script setup>
import { ref } from 'vue';
import { deleteVehicle } from '../services/vehicles';
import { shareVehicle } from '../services/vehicleShares';
import { useAuth } from '../composables/useAuth';

const props = defineProps({
  vehicle: Object,
});

const emit = defineEmits(['delete', 'select', 'share']);

const { token } = useAuth();
const showConfirm = ref(false);
const deleting = ref(false);
const showShareForm = ref(false);
const shareEmail = ref('');
const sharePermission = ref('read');
const shareLoading = ref(false);
const shareError = ref('');

async function handleDelete() {
  deleting.value = true;
  try {
    await deleteVehicle(props.vehicle.id, token.value);
    emit('delete');
  } catch (e) {
    console.error(e);
  }
}

async function handleShare() {
  if (!shareEmail.value) {
    shareError.value = 'Please enter an email address';
    return;
  }

  shareLoading.value = true;
  shareError.value = '';

  try {
    await shareVehicle(props.vehicle.id, shareEmail.value, sharePermission.value, token.value);
    shareEmail.value = '';
    sharePermission.value = 'read';
    showShareForm.value = false;
    emit('share');
  } catch (e) {
    shareError.value = e.message;
  } finally {
    shareLoading.value = false;
  }
}

function getPermissionBadgeClass() {
  if (props.vehicle.permission_level === 'owner') return 'badge-owner';
  if (props.vehicle.permission_level === 'write') return 'badge-write';
  return 'badge-read';
}

function getPermissionLabel() {
  if (props.vehicle.permission_level === 'owner') return 'Owner';
  if (props.vehicle.permission_level === 'write') return 'Can Write';
  return 'Read Only';
}
</script>

<template>
  <li class="vehicle-item">
    <div class="vehicle-info" @click="emit('select', vehicle)">
      <div class="vehicle-header">
        <strong>{{ vehicle.make }} {{ vehicle.model }}</strong>
        <span :class="['permission-badge', getPermissionBadgeClass()]">
          {{ getPermissionLabel() }}
        </span>
      </div>
      <span class="registration">{{ vehicle.registration }}</span>
    </div>
    <div class="vehicle-actions">
      <template v-if="!vehicle.is_shared">
        <button v-if="!showShareForm" @click="showShareForm = true">Share</button>
        <template v-else>
          <div class="share-form">
            <input 
              type="email" 
              v-model="shareEmail" 
              placeholder="Enter email"
              :disabled="shareLoading"
            />
            <select v-model="sharePermission" :disabled="shareLoading">
              <option value="read">Read</option>
              <option value="write">Write</option>
            </select>
            <button @click="handleShare" :disabled="shareLoading">
              {{ shareLoading ? 'Sharing...' : 'Share' }}
            </button>
            <button @click="showShareForm = false" :disabled="shareLoading">Cancel</button>
            <p v-if="shareError" class="error">{{ shareError }}</p>
          </div>
        </template>
      </template>
      
      <template v-if="!vehicle.is_shared && !showShareForm">
        <template v-if="showConfirm">
          <span>Confirm? </span>
          <button @click="handleDelete" :disabled="deleting">
            {{ deleting ? 'Deleting...' : 'Yes' }}
          </button>
          <button @click="showConfirm = false" :disabled="deleting">No</button>
        </template>
        <button v-else class="delete-btn" @click="showConfirm = true">Delete</button>
      </template>
    </div>
  </li>
</template>

<style scoped>
.vehicle-item {
  display: flex;
  justify-content: space-between;
  align-items: flex-start;
  padding: 1rem;
  border-bottom: 1px solid #e0e0e0;
}

.vehicle-info {
  flex: 1;
  cursor: pointer;
}

.vehicle-header {
  display: flex;
  align-items: center;
  gap: 0.5rem;
}

.permission-badge {
  font-size: 0.75rem;
  padding: 0.25rem 0.5rem;
  border-radius: 3px;
  font-weight: 500;
}

.badge-owner {
  background-color: #4CAF50;
  color: white;
}

.badge-write {
  background-color: #2196F3;
  color: white;
}

.badge-read {
  background-color: #9E9E9E;
  color: white;
}

.registration {
  color: #666;
  font-size: 0.9rem;
  margin-left: 0.5rem;
}

.vehicle-actions {
  display: flex;
  gap: 0.5rem;
  align-items: center;
}

.share-form {
  display: flex;
  flex-direction: column;
  gap: 0.5rem;
}

.share-form input,
.share-form select {
  padding: 0.25rem;
  font-size: 0.9rem;
}

.delete-btn {
  background-color: #f44336;
  color: white;
}

.error {
  color: #f44336;
  font-size: 0.8rem;
  margin: 0;
}
</style>
