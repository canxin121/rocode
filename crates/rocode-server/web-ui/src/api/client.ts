// ── API Client ─────────────────────────────────────────────────────────────

export class ApiError extends Error {
  constructor(
    public readonly status: number,
    public readonly statusText: string,
    public readonly body: string,
  ) {
    super(`${status} ${statusText}: ${body}`);
    this.name = "ApiError";
  }
}

export async function api(path: string, options: RequestInit = {}): Promise<Response> {
  const response = await fetch(path, {
    headers: {
      "Content-Type": "application/json",
      ...(options.headers as Record<string, string> | undefined),
    },
    ...options,
  });
  if (!response.ok) {
    const text = await response.text();
    throw new ApiError(response.status, response.statusText, text);
  }
  return response;
}

export async function apiJson<T>(path: string, options: RequestInit = {}): Promise<T> {
  const response = await api(path, options);
  return response.json() as Promise<T>;
}
