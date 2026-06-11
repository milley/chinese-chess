<template>
  <div class="container">
    <div class="card" style="max-width: 400px; margin: 100px auto;">
      <h2 style="text-align: center; margin-bottom: 24px;">中国象棋 - 登录</h2>
      <form @submit.prevent="handleLogin">
        <div class="form-group">
          <label>用户名</label>
          <input v-model="username" type="text" placeholder="请输入用户名" required />
        </div>
        <div class="form-group">
          <label>密码</label>
          <input v-model="password" type="password" placeholder="请输入密码" required />
        </div>
        <div v-if="error" class="error-message">{{ error }}</div>
        <button class="btn btn-primary" style="width: 100%; margin-top: 16px;" type="submit">登录</button>
      </form>
      <p style="text-align: center; margin-top: 16px;">
        没有账号？<router-link to="/register">注册</router-link>
      </p>
    </div>
  </div>
</template>

<script setup lang="ts">
import { ref } from 'vue';
import { useRouter, useRoute } from 'vue-router';
import { useUserStore } from '../stores/user';

const router = useRouter();
const route = useRoute();
const userStore = useUserStore();

const username = ref('');
const password = ref('');
const error = ref('');

async function handleLogin() {
  error.value = '';
  try {
    await userStore.login(username.value, password.value);
    const redirect = (route.query.redirect as string) || '/lobby';
    router.push(redirect);
  } catch (err: any) {
    error.value = err.response?.data?.error || '登录失败';
  }
}
</script>