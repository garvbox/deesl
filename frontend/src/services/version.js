export async function getVersion() {
  const response = await fetch('/api/version');
  if (!response.ok) {
    throw new Error('Failed to fetch version');
  }
  return response.json();
}
