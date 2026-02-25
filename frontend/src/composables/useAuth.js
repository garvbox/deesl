import { ref, computed } from 'vue';
import { apiGet } from '../services/api';
import router from '../router';

const user = ref(null);
const loading = ref(true);

async function fetchCurrentUser() {
  try {
    const response = await fetch('/api/auth/me', {
      credentials: 'include',
    });
    if (response.ok) {
      const data = await response.json();
      user.value = {
        userId: data.user_id,
        email: data.email,
      };
      return true;
    }
  } catch (e) {
    console.error('Failed to fetch current user:', e);
  }
  user.value = null;
  return false;
}

export function useAuth() {
  const isLoggedIn = computed(() => user.value !== null);
  const email = computed(() => user.value?.email);
  const userId = computed(() => user.value?.userId);

  // Check auth status on initialization
  async function init() {
    loading.value = true;
    await fetchCurrentUser();
    loading.value = false;
  }

  // For OAuth redirect handling - now just checks if user is logged in
  async function initFromRedirect() {
    // Just verify auth state by calling the API
    await fetchCurrentUser();
  }

  function logout() {
    user.value = null;
    // Clear auth cookie by calling logout endpoint (optional)
    fetch('/api/auth/logout', { method: 'POST', credentials: 'include' }).catch(() => {});
  }

  return {
    user,
    isLoggedIn,
    email,
    userId,
    loading: computed(() => loading.value),
    init,
    initFromRedirect,
    logout,
  };
}
