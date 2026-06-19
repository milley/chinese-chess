import { createRouter, createWebHistory } from 'vue-router';

const router = createRouter({
  history: createWebHistory(),
  routes: [
    { path: '/', redirect: '/lobby' },
    {
      path: '/login',
      name: 'Login',
      component: () => import('../views/LoginView.vue'),
      meta: { guest: true },
    },
    {
      path: '/register',
      name: 'Register',
      component: () => import('../views/RegisterView.vue'),
      meta: { guest: true },
    },
    {
      path: '/lobby',
      name: 'Lobby',
      component: () => import('../views/LobbyView.vue'),
      meta: { requiresAuth: true },
    },
    {
      path: '/game/:id',
      name: 'Game',
      component: () => import('../views/GameView.vue'),
      meta: { requiresAuth: true },
    },
    {
      path: '/replay/:id',
      name: 'Replay',
      component: () => import('../views/ReplayView.vue'),
      meta: { requiresAuth: true },
    },
    {
      path: '/profile',
      name: 'Profile',
      component: () => import('../views/ProfileView.vue'),
      meta: { requiresAuth: true },
    },
  ],
});

// Auth guard
router.beforeEach(async (to, _from, next) => {
  const { useUserStore } = await import('../stores/user');
  const userStore = useUserStore();

  if (userStore.isLoggedIn && !userStore.user) {
    await userStore.init();
  }

  if (to.meta.requiresAuth && !userStore.isLoggedIn) {
    next({ name: 'Login', query: { redirect: to.fullPath } });
  } else if (to.meta.guest && userStore.isLoggedIn) {
    next({ name: 'Lobby' });
  } else {
    next();
  }
});

export default router;