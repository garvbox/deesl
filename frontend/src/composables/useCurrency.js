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

  async function loadCurrency() {
    try {
      const profile = await apiGet('/users/me');
      currency.value = profile.currency;
      localStorage.setItem('currency', profile.currency);
    } catch (e) {
      console.error('Failed to load user currency', e);
    }
  }

  async function saveCurrency(newCurrency) {
    const profile = await apiPatch('/users/me', { currency: newCurrency });
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
