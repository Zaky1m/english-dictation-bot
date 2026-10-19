const tg = window.Telegram?.WebApp;

export class ApiError extends Error {
  constructor(status, code) {
    super(code);
    this.status = status;
    this.code = code;
  }
}

/** JSON request authenticated with Telegram initData. Throws ApiError. */
export async function api(method, path, body, signal) {
  const res = await fetch(path, {
    method,
    signal,
    headers: {
      authorization: `tma ${tg?.initData ?? ''}`,
      ...(body === undefined ? {} : { 'content-type': 'application/json' }),
    },
    body: body === undefined ? undefined : JSON.stringify(body),
  });
  if (res.status === 204) return null;
  const data = await res.json().catch(() => ({}));
  if (!res.ok) throw new ApiError(res.status, data.error ?? 'error');
  return data;
}
