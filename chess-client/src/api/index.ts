import axios, { type AxiosInstance } from 'axios';
import type {
  LoginRequest,
  CreateUser,
  LoginResponse,
  User,
  UpdateUserRequest,
  Game,
  CreateGameRequest,
  CreateGameResponse,
  MakeMoveResponse,
  ValidMovesRequest,
  ValidMovesResponse,
  AiMoveRequest,
  AiMoveResponse,
  MoveEntry,
} from '../types';

const API_BASE_URL = import.meta.env.VITE_API_URL || '';

// Lazy-initialized store reference to avoid circular dependency.
// The 401 interceptor needs the user store for proper logout cleanup
// (Pinia store + WS disconnect), but importing the store directly at
// module level creates a circular dependency (store -> api -> store).
let _logoutFn: (() => void) | null = null;

/// Register the logout function from the user store.
/// Called once after the store is created.
export function registerLogoutFn(fn: () => void) {
  _logoutFn = fn;
}

class ApiService {
  private client: AxiosInstance;

  constructor() {
    this.client = axios.create({
      baseURL: API_BASE_URL,
      headers: { 'Content-Type': 'application/json' },
    });

    this.client.interceptors.request.use((config) => {
      const token = localStorage.getItem('token');
      if (token) {
        config.headers.Authorization = `Bearer ${token}`;
      }
      return config;
    });

    this.client.interceptors.response.use(
      (response) => response,
      (error) => {
        if (error.response?.status === 429) {
          // Rate limited — show friendly message
          const retryAfter = error.response.headers['retry-after'];
          const message = retryAfter
            ? `操作过于频繁，请${retryAfter}秒后再试`
            : '操作过于频繁，请稍后再试';
          alert(message);
          return Promise.reject(error);
        }
        if (error.response?.status === 401) {
          // Use the user store's logout function for proper cleanup
          // (clears Pinia state, disconnects WebSocket) instead of
          // raw localStorage + window.location.href which bypasses both.
          if (_logoutFn) {
            _logoutFn();
          } else {
            // Fallback if store not yet registered (shouldn't happen in normal flow)
            localStorage.removeItem('token');
            localStorage.removeItem('user');
          }
          window.location.href = '/login';
        }
        return Promise.reject(error);
      }
    );
  }

  // 用户 API
  async register(data: CreateUser): Promise<LoginResponse> {
    const res = await this.client.post<LoginResponse>('/api/users', data);
    return res.data;
  }

  async login(data: LoginRequest): Promise<LoginResponse> {
    const res = await this.client.post<LoginResponse>('/api/users/login', data);
    return res.data;
  }

  async getCurrentUser(): Promise<User> {
    const res = await this.client.get<User>('/api/users/me');
    return res.data;
  }

  async updateUser(data: UpdateUserRequest): Promise<User> {
    const res = await this.client.put<User>('/api/users/me', data);
    return res.data;
  }

  async deleteUser(): Promise<void> {
    await this.client.delete('/api/users/me');
  }

  async getUser(id: string): Promise<User> {
    const res = await this.client.get<User>(`/api/users/${id}`);
    return res.data;
  }

  async listUsers(page?: number, pageSize?: number): Promise<User[]> {
    const res = await this.client.get<User[]>('/api/users', {
      params: { page, page_size: pageSize },
    });
    return res.data;
  }

  // 对局 API
  async createGame(data: CreateGameRequest): Promise<CreateGameResponse> {
    const res = await this.client.post<CreateGameResponse>('/api/games', data);
    return res.data;
  }

  async getGame(id: string): Promise<Game> {
    const res = await this.client.get<Game>(`/api/games/${id}`);
    return res.data;
  }

  async listGames(status?: string, page?: number, pageSize?: number): Promise<Game[]> {
    const res = await this.client.get<Game[]>('/api/games', {
      params: { status, page, page_size: pageSize },
    });
    return res.data;
  }

  async joinGame(id: string): Promise<Game> {
    const res = await this.client.post<Game>(`/api/games/${id}/join`);
    return res.data;
  }

  async deleteGame(id: string): Promise<void> {
    await this.client.delete(`/api/games/${id}`);
  }

  async rematch(gameId: string): Promise<{ game_id: string; color: string }> {
    const res = await this.client.post<{ game_id: string; color: string }>(`/api/games/${gameId}/rematch`);
    return res.data;
  }

  // AI API
  async getAiMove(data: AiMoveRequest): Promise<AiMoveResponse> {
    const res = await this.client.post<AiMoveResponse>('/api/ai/move', data);
    return res.data;
  }

  // 走法 API
  async getValidMoves(fen: string, from: string): Promise<string[]> {
    const data: ValidMovesRequest = { fen, from };
    const res = await this.client.post<ValidMovesResponse>('/api/moves/valid', data);
    return res.data.moves;
  }

  async makeMove(gameId: string, from: string, to: string): Promise<MakeMoveResponse> {
    const res = await this.client.post<MakeMoveResponse>(`/api/games/${gameId}/move`, { from, to });
    return res.data;
  }

  async getGameMoves(gameId: string): Promise<MoveEntry[]> {
    const res = await this.client.get<MoveEntry[]>(`/api/games/${gameId}/moves`);
    return res.data;
  }
}

export const api = new ApiService();
