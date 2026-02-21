import { ref, computed } from 'vue';
import { apiPost } from '../services/api';

const user = ref(getStoredAuth());

function getStoredAuth() {
  const token = localStorage.getItem('auth_token');
  const userId = localStorage.getItem('user_id');
  const email = localStorage.getItem('user_email');
  
  if (token && userId && email) {
    return { token, userId: parseInt(userId, 10), email };
  }
  return null;
}

function storeAuth(auth) {
  localStorage.setItem('auth_token', auth.token);
  localStorage.setItem('user_id', auth.user_id.toString());
  localStorage.setItem('user_email', auth.email);
}

function clearAuth() {
  localStorage.removeItem('auth_token');
  localStorage.removeItem('user_id');
  localStorage.removeItem('user_email');
}

export function useAuth() {
  const isLoggedIn = computed(() => user.value !== null);
  const token = computed(() => user.value?.token);
  const email = computed(() => user.value?.email);
  const userId = computed(() => user.value?.userId);

  async function login(emailVal, passwordVal) {
    const response = await apiPost('/auth/login', { email: emailVal, password: passwordVal });
    user.value = { 
      token: response.token, 
      userId: response.user_id, 
      email: response.email 
    };
    storeAuth(response);
  }

  async function register(emailVal, passwordVal) {
    const response = await apiPost('/auth/register', { email: emailVal, password: passwordVal });
    user.value = { 
      token: response.token, 
      userId: response.user_id, 
      email: response.email 
    };
    storeAuth(response);
  }

  function logout() {
    user.value = null;
    clearAuth();
  }

  return {
    user,
    isLoggedIn,
    token,
    email,
    userId,
    login,
    register,
    logout,
  };
}
