import { createRouter, createWebHistory } from 'vue-router';
import Dashboard from './components/Dashboard.vue';
import VehiclesPage from './components/VehiclesPage.vue';

const routes = [
  { path: '/', component: Dashboard },
  { path: '/vehicles', component: VehiclesPage },
];

export default createRouter({
  history: createWebHistory(),
  routes,
});
