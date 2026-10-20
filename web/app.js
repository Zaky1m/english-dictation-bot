import { api, ApiError, leaveIfNotPublic } from './api.js';
import { languages, loadLanguage, t } from './i18n.js';
import { h, toast } from './dom.js';
import { showExercise } from './exercise.js';
import { showMap } from './map.js';
import { showResult } from './result.js';
import { showLimit } from './limit.js';
import { showChallenge } from './challenge.js';
import { showProfile } from './profile.js';
import { showSettings } from './settings.js';
import { showPro } from './pro.js';
import { showWords } from './words.js';
import { showLeague } from './league.js';

const tg = window.Telegram?.WebApp;
const root = document.getElementById('app');
const sessions = new Map();

const ctx = {
  me: null,
  leaveHandlers: [],
  onLeave(fn) { this.leaveHandlers.push(fn); },
  go(hash) { location.hash = hash; },
  /** Running totals for a lesson, shown on its result screen. */
  recordSolved(key, data) {
    const s = sessions.get(key) ?? { score: 0, solved: 0, clean: 0, badges: [], startedAt: Date.now() };
    s.score += data.score.score;
    s.solved += 1;
    s.clean += data.score.accuracy === 1 ? 1 : 0;
    s.badges.push(...data.new_badges);
    sessions.set(key, s);
  },
  takeSession(key) {
    const s = sessions.get(key);
    sessions.delete(key);
    return s ? { ...s, ms: Date.now() - s.startedAt } : null;
  },
};

function applySafeArea() {
  const inset = tg?.safeAreaInset ?? {};
  const content = tg?.contentSafeAreaInset ?? {};
  document.documentElement.style.setProperty('--safe-top', `${(inset.top ?? 0) + (content.top ?? 0)}px`);
  document.documentElement.style.setProperty('--safe-bottom', `${inset.bottom ?? 0}px`);
}

function message(text) {
  root.replaceChildren(h('div', { class: 'center-screen' }, text));
}

const sources = {
  lesson: (id) => ({
    key: `lesson:${id}`,
    next: () => api('POST', `/api/lessons/${id}/next`),
    complete: () => ctx.go(`#/result/${id}`),
  }),
  review: () => ({
    key: 'review',
    next: () => api('POST', '/api/review/next'),
    complete: (r) => {
      if (r.none) toast(t('no_reviews'));
      ctx.go('#/');
    },
  }),
  challenge: () => ({
    key: 'challenge',
    next: () => api('POST', '/api/challenge/next'),
    complete: () => ctx.go('#/challenge'),
  }),
};

async function route() {
  ctx.leaveHandlers.splice(0).forEach((fn) => fn());
  const [, name, arg] = location.hash.split('/');
  try {
    if (name === 'lesson' && arg) return await showExercise(root, sources.lesson(Number(arg)), ctx);
    if (name === 'review') return await showExercise(root, sources.review(), ctx);
    if (name === 'challenge' && arg === 'play') return await showExercise(root, sources.challenge(), ctx);
    if (name === 'challenge') return await showChallenge(root, ctx);
    if (name === 'result' && arg) return await showResult(root, Number(arg), ctx);
    if (name === 'limit') return await showLimit(root, ctx);
    if (name === 'profile') return await showProfile(root, ctx);
    if (name === 'league') return await showLeague(root, ctx);
    if (name === 'settings') return await showSettings(root, ctx);
    if (name === 'pro') return await showPro(root, ctx);
    if (name === 'words') return await showWords(root, ctx);
    return await showMap(root, ctx);
  } catch (e) {
    message(t(e instanceof ApiError && e.status === 401 ? 'error_auth' : 'error_generic'));
  }
}

/** Desktop clients get the whole window; phones already open full height. */
function useFullWindow() {
  tg?.expand();
  if (['tdesktop', 'macos', 'weba', 'webk'].includes(tg?.platform)) {
    try { tg.requestFullscreen?.(); } catch { /* older clients ignore it */ }
  }
}

async function start() {
  tg?.ready();
  useFullWindow();
  applySafeArea();
  tg?.onEvent?.('safeAreaChanged', applySafeArea);
  tg?.onEvent?.('contentSafeAreaChanged', applySafeArea);
  // The language list loads while the profile request is in flight.
  languages();
  try {
    ctx.me = await api('GET', '/api/me');
  } catch (e) {
    if (await leaveIfNotPublic(e)) return;
    await loadLanguage(tg?.initDataUnsafe?.user?.language_code);
    return message(t(e instanceof ApiError && e.status === 401 ? 'error_auth' : 'error_generic'));
  }
  await loadLanguage(ctx.me.lang);
  window.addEventListener('hashchange', route);
  route();
}

start();
