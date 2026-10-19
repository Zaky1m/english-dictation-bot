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

/** Development mode turns everyone but the admins away: back to the bot chat, silently. */
export async function leaveIfNotPublic(error) {
  if (!(error instanceof ApiError) || error.code !== 'not_public') return false;
  try {
    const { url } = await (await fetch('/api/bot-link')).json();
    if (url) tg?.openTelegramLink?.(url);
  } catch {
    // Closing alone still returns the user to Telegram.
  }
  tg?.close?.();
  return true;
}
