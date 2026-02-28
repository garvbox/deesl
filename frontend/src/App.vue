<script setup>
import { watch, onMounted } from 'vue';
import { RouterView, RouterLink } from 'vue-router';
import { useAuth } from './composables/useAuth';
import { useCurrency } from './composables/useCurrency';
import Login from './components/Login.vue';
import UserSettings from './components/UserSettings.vue';
import AppFooter from './components/AppFooter.vue';

const { isLoggedIn, email, userId, token, logout, initFromRedirect } = useAuth();
const { loadCurrency } = useCurrency();

onMounted(() => {
  initFromRedirect();
});

watch(
  isLoggedIn,
  (loggedIn) => {
    if (loggedIn) {
      loadCurrency(userId.value, token.value);
    }
  },
  { immediate: true },
);
</script>

<template>
  <div class="app">
    <header>
      <h1>Deesl Fuel Tracker</h1>
      <nav v-if="isLoggedIn" class="main-nav">
        <RouterLink to="/">Dashboard</RouterLink>
        <RouterLink to="/vehicles">Vehicles</RouterLink>
        <RouterLink to="/import">Import</RouterLink>
      </nav>
      <span v-if="isLoggedIn" class="user-info">{{ email }}</span>
      <span v-else>Please log in</span>
      <UserSettings v-if="isLoggedIn" />
      <button v-if="isLoggedIn" @click="logout">Logout</button>
    </header>
    <main>
      <RouterView v-if="isLoggedIn" />
      <div v-else class="auth-forms">
        <Login />
      </div>
    </main>
    <AppFooter />
  </div>
</template>

<style>
* {
  box-sizing: border-box;
  margin: 0;
  padding: 0;
}

body {
  font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, Oxygen, Ubuntu, sans-serif;
  background: #f5f5f5;
  min-height: 100vh;
}

.app {
  max-width: 600px;
  margin: 0 auto;
  padding: 16px;
}

header {
  display: flex;
  justify-content: space-between;
  align-items: center;
  padding: 16px 0;
  border-bottom: 1px solid #ddd;
  margin-bottom: 16px;
  flex-wrap: wrap;
  gap: 8px;
}

header h1 {
  font-size: 1.25rem;
  color: #333;
}

.main-nav {
  display: flex;
  gap: 12px;
}

.main-nav a {
  font-size: 0.875rem;
  color: #555;
  text-decoration: none;
  padding: 4px 8px;
  border-radius: 4px;
}

.main-nav a:hover {
  background: #f0f0f0;
  color: #333;
}

.main-nav a.router-link-active {
  color: #007bff;
  font-weight: 500;
}

header .user-info {
  font-size: 0.875rem;
  color: #666;
}

button {
  background: #007bff;
  color: white;
  border: none;
  padding: 8px 16px;
  border-radius: 4px;
  cursor: pointer;
  font-size: 0.875rem;
}

button:disabled {
  background: #ccc;
  cursor: not-allowed;
}

button.delete-btn {
  background: #dc3545;
}

button.delete-btn:hover:not(:disabled) {
  background: #c82333;
}

button:hover:not(:disabled) {
  background: #0056b3;
}

.login {
  background: white;
  padding: 24px;
  border-radius: 8px;
  box-shadow: 0 2px 4px rgba(0,0,0,0.1);
  max-width: 400px;
  margin: 40px auto;
}

.login h2 {
  margin-bottom: 16px;
  color: #333;
}

form label {
  display: block;
  margin-bottom: 12px;
}

form label span {
  display: block;
  margin-bottom: 4px;
  font-weight: 500;
  color: #555;
}

input[type="text"],
input[type="email"],
input[type="password"],
input[type="number"],
select {
  width: 100%;
  padding: 10px;
  border: 1px solid #ddd;
  border-radius: 4px;
  font-size: 16px;
}

input:focus,
select:focus {
  outline: none;
  border-color: #007bff;
}

form button {
  width: 100%;
  padding: 12px;
  margin-top: 8px;
}

.error {
  color: #dc3545;
  font-size: 0.875rem;
  margin-top: 8px;
}


.section-header {
  display: flex;
  justify-content: space-between;
  align-items: center;
  margin-bottom: 16px;
}

.section-header h2, .section-header h3 {
  font-size: 1.125rem;
  color: #333;
}

.add-vehicle-form, .add-entry-form {
  background: white;
  padding: 16px;
  border-radius: 8px;
  margin-bottom: 16px;
  box-shadow: 0 1px 3px rgba(0,0,0,0.1);
}

.add-vehicle-form label, .add-entry-form label {
  display: grid;
  grid-template-columns: 1fr;
  gap: 4px;
  margin-bottom: 12px;
  position: relative;
}

.vehicle-list, .entry-list {
  list-style: none;
  padding: 0;
}

.vehicle-item, .entry-item {
  background: white;
  padding: 16px;
  border-radius: 8px;
  margin-bottom: 8px;
  box-shadow: 0 1px 3px rgba(0,0,0,0.1);
  display: flex;
  justify-content: space-between;
  align-items: center;
  flex-wrap: wrap;
  gap: 8px;
}

.vehicle-info {
  cursor: pointer;
}

.vehicle-info:hover {
  color: #007bff;
}

.vehicle-info strong {
  display: block;
  font-size: 1rem;
}

.vehicle-info .registration {
  font-size: 0.875rem;
  color: #666;
}

.entry-info {
  display: flex;
  flex-wrap: wrap;
  gap: 8px;
  font-size: 0.875rem;
}

.entry-info .vehicle {
  font-weight: 500;
  color: #007bff;
}

.entry-info .date {
  color: #666;
  min-width: 140px;
}

.entry-info .mileage, .entry-info .litres, .entry-info .cost {
  font-weight: 500;
}

.entry-info .station {
  color: #666;
  font-style: italic;
}

.vehicle-actions, .entry-actions {
  font-size: 0.875rem;
}

.empty-state {
  text-align: center;
  color: #666;
  padding: 24px;
  background: white;
  border-radius: 8px;
}

.autocomplete-dropdown {
  position: absolute;
  top: 100%;
  left: 0;
  right: 0;
  background: white;
  border: 1px solid #ddd;
  border-radius: 4px;
  list-style: none;
  z-index: 10;
  max-height: 150px;
  overflow-y: auto;
  padding: 0;
}

.autocomplete-dropdown li {
  padding: 8px 12px;
  cursor: pointer;
}

.autocomplete-dropdown li:hover {
  background: #f0f0f0;
}

.recent-entries-section {
  margin-bottom: 24px;
}

@media (max-width: 480px) {
  .app {
    padding: 8px;
  }
  
  header {
    flex-direction: column;
    align-items: flex-start;
  }
  
  header h1 {
    font-size: 1rem;
  }
  
  .vehicle-item, .entry-item {
    flex-direction: column;
    align-items: flex-start;
  }
  
  .entry-info {
    width: 100%;
  }
  
  .vehicle-actions, .entry-actions {
    width: 100%;
    margin-top: 8px;
    display: flex;
    gap: 8px;
  }
  
  .login {
    margin: 16px auto;
    padding: 16px;
  }
}
</style>
