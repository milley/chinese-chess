import { describe, it, expect, beforeEach } from 'vitest';
import { useUserStore } from '../../stores/user';
import { createTestingPinia } from '@pinia/testing';

describe('userStore', () => {
  let store: ReturnType<typeof useUserStore>;

  beforeEach(() => {
    localStorage.clear();
    const pinia = createTestingPinia({ stubActions: false });
    store = useUserStore(pinia);
  });

  it('isLoggedIn is true when token is present', () => {
    store.token = 'some-jwt-token';
    expect(store.isLoggedIn).toBe(true);
  });

  it('isLoggedIn is false when token is null', () => {
    store.token = null;
    expect(store.isLoggedIn).toBe(false);
  });

  it('logout clears state and localStorage', () => {
    store.token = 'token';
    store.user = { id: '1', username: 'test', display_name: null, rating: 1500, wins: 0, losses: 0, draws: 0 };
    localStorage.setItem('token', 'token');
    localStorage.setItem('user', JSON.stringify(store.user));

    store.logout();

    expect(store.token).toBeNull();
    expect(store.user).toBeNull();
    expect(localStorage.getItem('token')).toBeNull();
    expect(localStorage.getItem('user')).toBeNull();
  });
});
