export interface UserRecord {
  id: string;
  email: string;
  name: string;
  passwordHash: string;
  createdAt: Date;
}

const users: UserRecord[] = [];

export async function findByEmail(email: string): Promise<UserRecord | null> {
  return users.find(u => u.email === email) || null;
}

export async function createUser(data: { email: string; name: string; passwordHash: string }): Promise<UserRecord> {
  const user: UserRecord = {
    id: Math.random().toString(36).substring(7),
    ...data,
    createdAt: new Date(),
  };
  users.push(user);
  return user;
}

export async function findById(id: string): Promise<UserRecord | null> {
  return users.find(u => u.id === id) || null;
}

export async function updateUser(id: string, updates: Partial<UserRecord>): Promise<UserRecord | null> {
  const idx = users.findIndex(u => u.id === id);
  if (idx === -1) return null;
  users[idx] = { ...users[idx], ...updates };
  return users[idx];
}
