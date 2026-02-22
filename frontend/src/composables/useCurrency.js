import { ref, computed } from 'vue';
import { apiGet, apiPatch } from '../services/api';

const SUPPORTED_CURRENCIES = ['EUR', 'GBP', 'USD', 'CAD', 'AUD'];

const currency = ref(localStorage.getItem('currency') || 'EUR');

export function useCurrency() {
  const formatter = computed(
    () =>
      new Intl.NumberFormat(undefined, {
        style: 'currency',
        currency: currency.value,
      }),
  );

  function formatCost(amount) {
    return formatter.value.format(amount);
  }

  async function loadCurrency(userId, token) {
    try {
      const profile = await apiGet(`/users/me?user_id=${userId}`, token);
      currency.value = profile.currency;
      localStorage.setItem('currency', profile.currency);
    } catch (e) {
      console.error('Failed to load user currency', e);
    }
  }

  async function saveCurrency(newCurrency, userId, token) {
    const profile = await apiPatch(`/users/me?user_id=${userId}`, { currency: newCurrency }, token);
    currency.value = profile.currency;
    localStorage.setItem('currency', profile.currency);
  }

  return {
    currency,
    supportedCurrencies: SUPPORTED_CURRENCIES,
    formatCost,
    loadCurrency,
    saveCurrency,
  };
}
