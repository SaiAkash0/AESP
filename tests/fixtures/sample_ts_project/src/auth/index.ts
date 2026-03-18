import { findByEmail, createUser } from '../db/users';
import * as bcrypt from 'bcrypt';
import * as jwt from 'jsonwebtoken';

export interface AuthResult {
  success: boolean;
  user?: User;
  token?: string;
  error?: string;
}

export interface User {
  id: string;
  email: string;
  name: string;
}

export async function authenticateUser(email: string, password: string): Promise<AuthResult> {
  const user = await findByEmail(email);
  if (!user) {
    return { success: false, error: 'User not found' };
  }

  const valid = await bcrypt.compare(password, user.passwordHash);
  if (!valid) {
    return { success: false, error: 'Invalid password' };
  }

  const token = generateToken(user);
  return { success: true, user, token };
}

export function generateToken(user: User): string {
  return jwt.sign(
    { id: user.id, email: user.email },
    process.env.JWT_SECRET || 'default-secret',
    { expiresIn: '24h' }
  );
}

export function verifyToken(token: string): User | null {
  try {
    const decoded = jwt.verify(token, process.env.JWT_SECRET || 'default-secret');
    return decoded as User;
  } catch {
    return null;
  }
}

export async function refreshToken(token: string): Promise<{ access: string; refresh: string }> {
  const decoded = jwt.verify(token, process.env.JWT_REFRESH_SECRET || 'refresh-secret');
  const user = await findByEmail((decoded as any).email);
  if (!user) throw new Error('User not found');

  const newAccess = generateToken(user);
  const newRefresh = jwt.sign({ email: user.email }, process.env.JWT_REFRESH_SECRET || 'refresh-secret');
  return { access: newAccess, refresh: newRefresh };
}
