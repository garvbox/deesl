import { ref, computed } from 'vue';

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

  // Picks up token/user_id/email query params placed by the OAuth callback redirect
  function initFromRedirect() {
    const params = new URLSearchParams(window.location.search);
    const redirectToken = params.get('token');
    const redirectUserId = params.get('user_id');
    const redirectEmail = params.get('email');

    if (redirectToken && redirectUserId && redirectEmail) {
      const auth = {
        token: redirectToken,
        user_id: parseInt(redirectUserId, 10),
        email: redirectEmail,
      };
      user.value = { token: auth.token, userId: auth.user_id, email: auth.email };
      storeAuth(auth);
      // Remove token from URL to prevent reuse on refresh
      window.history.replaceState({}, '', '/');
    }
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
    initFromRedirect,
    logout,
  };
}
