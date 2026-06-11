<template>
  <div class="container">
    <div class="card" style="max-width: 400px; margin: 60px auto;">
      <h2 style="text-align: center; margin-bottom: 24px;">中国象棋 - 注册</h2>
      <form @submit.prevent="handleRegister">
        <div class="form-group">
          <label>用户名</label>
          <input v-model="username" type="text" placeholder="3-20字符，字母开头" required />
        </div>
        <div class="form-group">
          <label>密码</label>
          <input v-model="password" type="password" placeholder="6-100字符" required />
        </div>
        <div class="form-group">
          <label>确认密码</label>
          <input v-model="confirmPassword" type="password" placeholder="再次输入密码" required />
        </div>
        <div class="form-group">
          <label>昵称 (可选)</label>
          <input v-model="displayName" type="text" placeholder="请输入昵称" />
        </div>
        <div v-if="error" class="error-message">{{ error }}</div>
        <button class="btn btn-primary" style="width: 100%; margin-top: 16px;" type="submit">注册</button>
      </form>
      <p style="text-align: center; margin-top: 16px;">
        已有账号？<router-link to="/login">登录</router-link>
      </p>
    </div>
  </div>
</template>

<script setup lang="ts">
import { ref } from 'vue';
import { useRouter } from 'vue-router';
import { useUserStore } from '../stores/user';

const router = useRouter();
const userStore = useUserStore();

const username = ref('');
const password = ref('');
const confirmPassword = ref('');
const displayName = ref('');
const error = ref('');

async function handleRegister() {
  error.value = '';

  if (password.value !== confirmPassword.value) {
    error.value = '两次密码不一致';
    return;
  }

  try {
    await userStore.register(username.value, password.value, displayName.value || undefined);
    router.push('/lobby');
  } catch (err: any) {
    error.value = err.response?.data?.error || '注册失败';
  }
}
</script>