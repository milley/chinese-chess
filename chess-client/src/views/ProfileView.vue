<template>
  <div>
    <nav class="nav-bar">
      <div>
        <router-link to="/lobby">大厅</router-link>
      </div>
    </nav>
    <div class="container">
      <div class="card" style="max-width: 500px; margin: 0 auto;">
        <h2 style="margin-bottom: 24px;">个人资料</h2>
        <div v-if="userStore.user">
          <div style="margin-bottom: 12px;">
            <strong>用户名：</strong>{{ userStore.user.username }}
          </div>
          <div style="margin-bottom: 12px;">
            <strong>昵称：</strong>{{ userStore.user.display_name || '未设置' }}
          </div>
          <div style="margin-bottom: 12px;">
            <strong>评分：</strong>{{ userStore.user.rating }}
          </div>
          <div style="margin-bottom: 12px;">
            <strong>战绩：</strong>{{ userStore.user.wins }}胜 {{ userStore.user.losses }}负 {{ userStore.user.draws }}和
          </div>

          <hr style="margin: 20px 0;" />

          <div class="form-group">
            <label>修改昵称</label>
            <input v-model="newDisplayName" type="text" placeholder="输入新昵称" />
          </div>
          <button class="btn btn-primary" @click="updateProfile" style="margin-right: 8px;">保存</button>

          <div style="margin-top: 24px;">
            <button class="btn btn-danger" @click="deleteAccount">删除账号</button>
          </div>
        </div>
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
import { ref } from 'vue';
import { useRouter } from 'vue-router';
import { useUserStore } from '../stores/user';

const router = useRouter();
const userStore = useUserStore();
const newDisplayName = ref(userStore.user?.display_name || '');

async function updateProfile() {
  try {
    await userStore.updateUser({ display_name: newDisplayName.value || undefined });
    alert('更新成功');
  } catch (err: any) {
    alert(err.response?.data?.error || '更新失败');
  }
}

async function deleteAccount() {
  if (!confirm('确定要删除账号吗？此操作不可撤销。')) return;
  try {
    await userStore.deleteUser();
    userStore.logout();
    router.push('/login');
  } catch (err: any) {
    alert(err.response?.data?.error || '删除失败');
  }
}
</script>