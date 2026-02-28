import { createRouter, createWebHistory } from 'vue-router';
import Dashboard from './components/Dashboard.vue';
import VehiclesPage from './components/VehiclesPage.vue';
import ImportFuelEntries from './components/ImportFuelEntries.vue';

const routes = [
  { path: '/', component: Dashboard },
  { path: '/vehicles', component: VehiclesPage },
  { path: '/import', component: ImportFuelEntries },
];

export default createRouter({
  history: createWebHistory(),
  routes,
});
