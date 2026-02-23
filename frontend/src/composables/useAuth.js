import { ref, computed } from 'vue';
import router from '../router';

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

    // Check if these exact params were already processed (prevents re-login on refresh after logout)
    const processedToken = sessionStorage.getItem('oauth_processed_token');
    if (processedToken && processedToken === redirectToken) {
      // Already processed these params, clean URL if needed and return
      if (redirectToken) {
        router.replace({ path: '/', query: {} });
      }
      return;
    }

    if (redirectToken && redirectUserId && redirectEmail) {
      const auth = {
        token: redirectToken,
        user_id: parseInt(redirectUserId, 10),
        email: redirectEmail,
      };
      user.value = { token: auth.token, userId: auth.user_id, email: auth.email };
      storeAuth(auth);
      // Mark these params as processed
      sessionStorage.setItem('oauth_processed_token', redirectToken);
      // Remove token from URL to prevent reuse on refresh - use router.replace for proper cleanup
      router.replace({ path: '/', query: {} });
    }
  }

  function logout() {
    user.value = null;
    clearAuth();
    // Clear the processed token flag so a fresh login will work
    sessionStorage.removeItem('oauth_processed_token');
    // Clean URL of any token params that might be present
    const params = new URLSearchParams(window.location.search);
    if (params.get('token')) {
      router.replace({ path: '/', query: {} });
    }
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
