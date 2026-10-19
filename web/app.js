import { api, ApiError } from './api.js';
import { loadLanguage, t } from './i18n.js';
import { h } from './dom.js';
import { showExercise } from './exercise.js';

const tg = window.Telegram?.WebApp;
const root = document.getElementById('app');
const ctx = {
  me: null,
  leaveHandlers: [],
  onLeave(fn) { this.leaveHandlers.push(fn); },
  go(hash) { location.hash = hash; },
};

function applySafeArea() {
  const inset = { ...(tg?.safeAreaInset ?? {}) };
  const content = tg?.contentSafeAreaInset ?? {};
  const top = (inset.top ?? 0) + (content.top ?? 0);
  document.documentElement.style.setProperty('--safe-top', `${top}px`);
  document.documentElement.style.setProperty('--safe-bottom', `${inset.bottom ?? 0}px`);
}

function message(text) {
  root.replaceChildren(h('div', { class: 'center-screen' }, text));
}

async function route() {
  ctx.leaveHandlers.splice(0).forEach((fn) => fn());
  const [, name, arg] = location.hash.split('/');
  if (name === 'lesson' && arg) return showExercise(root, Number(arg), ctx);
  if (name === 'result') return message(t('lesson_done'));
  // Temporary entry until the map screen exists: open the first available lesson.
  const map = await api('GET', '/api/map');
  const open = map.flatMap((s) => s.lessons).find((l) => l.state === 'open');
  if (open) location.replace(`#/lesson/${open.id}`);
  else message(t('lesson_done'));
}

async function start() {
  tg?.ready();
  tg?.expand();
  applySafeArea();
  tg?.onEvent?.('safeAreaChanged', applySafeArea);
  tg?.onEvent?.('contentSafeAreaChanged', applySafeArea);
  try {
    ctx.me = await api('GET', '/api/me');
  } catch (e) {
    await loadLanguage('ar');
    return message(t(e instanceof ApiError && e.status === 401 ? 'error_auth' : 'error_generic'));
  }
  await loadLanguage(ctx.me.lang);
  window.addEventListener('hashchange', route);
  route();
}

start();
