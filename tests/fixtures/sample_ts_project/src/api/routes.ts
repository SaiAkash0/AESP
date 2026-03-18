import { authenticateUser, refreshToken } from '../auth/index';

export interface ApiResponse {
  status: number;
  body: any;
}

export async function handleLogin(email: string, password: string): Promise<ApiResponse> {
  const result = await authenticateUser(email, password);
  if (result.success) {
    return { status: 200, body: { token: result.token, user: result.user } };
  }
  return { status: 401, body: { error: result.error } };
}

export async function handleRefresh(token: string): Promise<ApiResponse> {
  try {
    const tokens = await refreshToken(token);
    return { status: 200, body: tokens };
  } catch (err: any) {
    return { status: 401, body: { error: err.message } };
  }
}

export async function handleProtectedRoute(userId: string): Promise<ApiResponse> {
  return { status: 200, body: { message: `Hello user ${userId}` } };
}
