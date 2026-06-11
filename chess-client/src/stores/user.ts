import { ref, computed } from 'vue';
import { defineStore } from 'pinia';
import { api } from '../api';
import { wsService } from '../api/websocket';
import type { User } from '../types';

export const useUserStore = defineStore('user', () => {
  const token = ref<string | null>(localStorage.getItem('token'));
  const user = ref<User | null>(null);
  const isLoggedIn = computed(() => !!token.value);

  async function init() {
    if (token.value) {
      try {
        user.value = await api.getCurrentUser();
      } catch {
        logout();
      }
    }
  }

  async function register(username: string, password: string, displayName?: string) {
    const res = await api.register({ username, password, display_name: displayName });
    token.value = res.token;
    user.value = res.user;
    localStorage.setItem('token', res.token);
    localStorage.setItem('user', JSON.stringify(res.user));

    // Connect WebSocket
    try {
      await wsService.connect(res.token);
    } catch {
      // WS connection failed, but user is still logged in
    }
  }

  async function login(username: string, password: string) {
    const res = await api.login({ username, password });
    token.value = res.token;
    user.value = res.user;
    localStorage.setItem('token', res.token);
    localStorage.setItem('user', JSON.stringify(res.user));

    // Connect WebSocket
    try {
      await wsService.connect(res.token);
    } catch {
      // WS connection failed
    }
  }

  function logout() {
    token.value = null;
    user.value = null;
    localStorage.removeItem('token');
    localStorage.removeItem('user');
    wsService.disconnect();
  }

  async function updateUser(data: { display_name?: string }) {
    user.value = await api.updateUser(data);
    localStorage.setItem('user', JSON.stringify(user.value));
  }

  async function deleteUser() {
    await api.deleteUser();
    logout();
  }

  return { token, user, isLoggedIn, init, register, login, logout, updateUser, deleteUser };
});