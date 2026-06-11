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
} from '../types';

const API_BASE_URL = import.meta.env.VITE_API_URL || '';

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
        if (error.response?.status === 401) {
          localStorage.removeItem('token');
          localStorage.removeItem('user');
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
}

export const api = new ApiService();
