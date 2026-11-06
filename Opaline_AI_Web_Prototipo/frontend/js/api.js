const API = 'http://127.0.0.1:8090/api/v1';
function getToken() { return localStorage.getItem('access_token'); }
function setToken(t, r) { localStorage.setItem('access_token', t); if (r) localStorage.setItem('refresh_token', r); }
function clearToken() { localStorage.removeItem('access_token'); localStorage.removeItem('refresh_token'); }
async function api(path, opts = {}) {
  const headers = { 'Content-Type': 'application/json', 'Accept': 'application/json', ...(opts.headers || {}) };
  const t = getToken();
  if (t) headers['Authorization'] = 'Bearer ' + t;
  const res = await fetch(API + path, { ...opts, headers });
  let data = null;
  try { data = await res.json(); } catch (e) { data = null; }
  if (!res.ok) throw { status: res.status, data };
  return data;
}
window.OpalineAPI = { API, getToken, setToken, clearToken, api };
