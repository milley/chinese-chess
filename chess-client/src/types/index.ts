// User types
export interface User {
  id: string;
  username: string;
  display_name: string | null;
  rating: number;
  wins: number;
  losses: number;
  draws: number;
}

export interface LoginRequest {
  username: string;
  password: string;
}

export interface CreateUser {
  username: string;
  password: string;
  display_name?: string;
}

export interface LoginResponse {
  token: string;
  user: User;
}

export interface UpdateUserRequest {
  display_name?: string;
}

// Game types
export type GameStatus = 'waiting' | 'playing' | 'finished';
export type GameResult = 'red_win' | 'black_win' | 'draw';

export interface Game {
  id: string;
  red_player: User | null;
  black_player: User | null;
  status: GameStatus;
  result: GameResult | null;
  end_reason: string | null;
  fen: string;
  time_control: number | null;
  move_time_limit: number | null;
  byoyomi: number | null;
  red_time: number | null;
  black_time: number | null;
  created_at: string;
}

export interface CreateGameRequest {
  player_color?: string;
  time_control?: number;
  move_time_limit?: number;
  byoyomi?: number;
}

export interface CreateGameResponse {
  game_id: string;
  color: string;
}

export interface MakeMoveRequest {
  from: string;
  to: string;
}

export interface MakeMoveResponse {
  fen: string;
  is_check: boolean;
  is_game_over: boolean;
  result: string | null;
  end_reason: string | null;
}

export interface ValidMovesRequest {
  fen: string;
  from: string;
}

export interface ValidMovesResponse {
  moves: string[];
}

export interface AiMoveRequest {
  fen: string;
  depth?: number;
}

export interface AiMoveResponse {
  best_move: string | null;
  depth: number;
}

// WebSocket message types
export interface WsAuthMessage {
  type: 'auth';
  token: string;
}

export interface WsJoinGameMessage {
  type: 'join_game';
  game_id: string;
}

export interface WsLeaveGameMessage {
  type: 'leave_game';
  game_id: string;
}

export interface WsMakeMoveMessage {
  type: 'make_move';
  game_id: string;
  from: string;
  to: string;
}

export interface WsResignMessage {
  type: 'resign';
  game_id: string;
}

export interface WsOfferDrawMessage {
  type: 'offer_draw';
  game_id: string;
}

export interface WsRespondDrawMessage {
  type: 'respond_draw';
  game_id: string;
  accept: boolean;
}

export interface WsPingMessage {
  type: 'ping';
}

export type WsClientMessage =
  | WsAuthMessage
  | WsJoinGameMessage
  | WsLeaveGameMessage
  | WsMakeMoveMessage
  | WsResignMessage
  | WsOfferDrawMessage
  | WsRespondDrawMessage
  | WsPingMessage;

export interface WsJoinedGameMessage {
  type: 'joined_game';
  game_id: string;
  color: string;
  fen: string;
}

export interface WsOpponentJoinedMessage {
  type: 'opponent_joined';
  game_id: string;
  opponent: User;
  fen: string;
}

export interface WsMoveMadeMessage {
  type: 'move_made';
  game_id: string;
  from: string;
  to: string;
  fen: string;
  is_check: boolean;
  red_time?: number;
  black_time?: number;
}

export interface WsIllegalMoveMessage {
  type: 'illegal_move';
  game_id: string;
  reason: string;
}

export interface WsGameOverMessage {
  type: 'game_over';
  game_id: string;
  result: string;
  reason: string;
}

export interface WsOpponentDisconnectedMessage {
  type: 'opponent_disconnected';
  game_id: string;
}

export interface WsDrawOfferedMessage {
  type: 'draw_offered';
  game_id: string;
}

export interface WsDrawResponseMessage {
  type: 'draw_response';
  game_id: string;
  accepted: boolean;
}

export interface WsTimeUpdateMessage {
  type: 'time_update';
  game_id: string;
  red_time: number;
  black_time: number;
  active_color: string;
  red_in_byoyomi: boolean;
  black_in_byoyomi: boolean;
}

export interface WsPongMessage {
  type: 'pong';
}

export interface WsErrorMessage {
  type: 'error';
  message: string;
}

export type WsServerMessage =
  | WsJoinedGameMessage
  | WsOpponentJoinedMessage
  | WsMoveMadeMessage
  | WsIllegalMoveMessage
  | WsGameOverMessage
  | WsOpponentDisconnectedMessage
  | WsDrawOfferedMessage
  | WsDrawResponseMessage
  | WsTimeUpdateMessage
  | WsPongMessage
  | WsErrorMessage;
