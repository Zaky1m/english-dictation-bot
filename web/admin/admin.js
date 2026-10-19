import { api, ApiError } from '../api.js';
import { loadLanguage, t } from '../i18n.js';
import { h, toast } from '../dom.js';
import { SITUATION_ICONS } from '../icons.js';

const tg = window.Telegram?.WebApp;
const root = document.getElementById('app');
const state = { tab: 'overview', me: null, refresh: null };
const TABS = ['overview', 'users', 'payments', 'settings', 'content'];

const date = (value) => (value ? new Date(value).toLocaleDateString() : '');

async function act(fn, done) {
  try {
    await fn();
    if (done) toast(done);
  } catch (e) {
    toast(e instanceof ApiError ? `${t('admin_error')}: ${e.code}` : t('error_generic'));
  }
}

function frame(content) {
  root.replaceChildren(
    h('div', { class: 'top' }, h('h1', { class: 'title grow' }, t('admin_title'))),
    h('div', { class: 'tabs' }, TABS.map((tab) => h('button', {
      class: state.tab === tab ? 'on' : '',
      onclick: () => { state.tab = tab; show(); },
    }, t(`admin_tab_${tab}`)))),
    h('div', { class: 'panel' }, content));
}

async function overview() {
  const o = await api('GET', '/api/admin/overview');
  const card = (value, key) => h('div', { class: 'card center' }, h('div', { class: 'big latin' }, value), h('div', { class: 'muted small' }, t(key)));
  return [h('div', { class: 'stats' },
    card(o.users, 'admin_users'), card(o.new_users_7d, 'admin_new_users'), card(o.active_today, 'admin_active_today'),
    card(o.pro, 'admin_pro_users'), card(o.sentences_today, 'admin_sentences_today'), card(o.sentences_total, 'admin_sentences_total'),
    card(o.stars_total, 'admin_stars_total'))];
}

async function users() {
  const results = h('div', { class: 'card full' });
  const input = h('input', { type: 'search', placeholder: t('admin_search_hint'), 'aria-label': t('admin_search_hint') });

  async function search() {
    const rows = await api('GET', `/api/admin/users?q=${encodeURIComponent(input.value)}`);
    results.replaceChildren(...(rows.length ? rows.map(userRow) : [h('div', { class: 'muted' }, t('admin_none'))]));
  }

  function userRow(u) {
    const days = h('input', { type: 'number', min: '1', max: '3650', placeholder: t('admin_days'), style: 'width:90px' });
    const grant = (plan, extra = {}) => act(async () => { await api('POST', `/api/admin/users/${u.id}/grant`, { plan, ...extra }); await search(); }, t('admin_done'));
    return h('div', { class: 'rowline' },
      h('div', { class: 'grow' }, h('b', {}, u.name || '-'), ' ', h('span', { class: 'muted latin small' }, `${u.username ? `@${u.username} · ` : ''}${u.id}`)),
      h('span', { class: 'status' }, u.pro_until ? `Pro ${date(u.pro_until)}` : t('plan_free')),
      h('button', { class: 'btn s', onclick: () => grant('month') }, t('plan_month')),
      h('button', { class: 'btn s', onclick: () => grant('year') }, t('plan_year')),
      days,
      h('button', { class: 'btn2 s', onclick: () => grant('custom', { days: Number(days.value) }) }, t('admin_grant_days')),
      u.pro_until ? h('button', { class: 'btn2 s', onclick: () => act(async () => { await api('POST', `/api/admin/users/${u.id}/revoke`); await search(); }, t('admin_done')) }, t('admin_revoke')) : null);
  }

  input.addEventListener('keydown', (e) => { if (e.key === 'Enter') search(); });
  await search();
  return [h('div', { class: 'field' }, input, h('button', { class: 'btn s', onclick: search }, t('admin_search'))), results];
}

async function payments() {
  const data = await api('GET', '/api/admin/payments');
  return [
    h('div', { class: 'card full' }, h('b', {}, t('admin_payments')),
      ...(data.payments.length ? data.payments.map((p) => h('div', { class: 'rowline' },
        h('div', { class: 'grow' }, p.name, ' ', h('span', { class: 'muted latin small' }, `${p.user_id} · ${p.payload}`)),
        h('span', { class: 'latin' }, `${p.amount} ${p.currency}`), h('span', { class: 'muted small' }, date(p.created_at)))) : [h('div', { class: 'muted' }, t('admin_none'))])),
    h('div', { class: 'card full' }, h('b', {}, t('admin_grants')),
      ...(data.grants.length ? data.grants.map((g) => h('div', { class: 'rowline' },
        h('div', { class: 'grow' }, g.name, ' ', h('span', { class: 'muted latin small' }, g.user_id)),
        h('span', { class: 'status' }, g.plan), h('span', { class: 'muted small latin' }, `${date(g.starts_at)} - ${date(g.ends_at)}`))) : [h('div', { class: 'muted' }, t('admin_none'))]))];
}

async function settings() {
  const rows = await api('GET', '/api/admin/settings');
  return [h('div', { class: 'card full' }, ...rows.map(([key, value]) => {
    let read;
    let control;
    if (typeof value === 'boolean') {
      control = h('input', { type: 'checkbox', ...(value ? { checked: '' } : {}) });
      read = () => control.checked;
    } else if (Array.isArray(value)) {
      control = h('input', { type: 'text', value: value.join(', '), dir: 'ltr' });
      read = () => control.value.split(',').map((x) => Number(x.trim())).filter((x) => !Number.isNaN(x));
    } else if (typeof value === 'number') {
      control = h('input', { type: 'number', step: 'any', value: String(value) });
      read = () => Number(control.value);
    } else {
      control = h('input', { type: 'text', value: String(value) });
      read = () => control.value;
    }
    return h('div', { class: 'rowline' }, h('div', { class: 'key' }, key), control,
      h('button', { class: 'btn s', onclick: () => act(() => api('PUT', `/api/admin/settings/${key}`, { value: read() }), t('admin_saved')) }, t('admin_save')));
  }))];
}

async function sentencesView(situation) {
  const rows = await api('GET', `/api/admin/situations/${situation.id}/sentences`);
  const list = h('div', { class: 'card full' }, h('div', { class: 'row' },
    h('button', { class: 'btn2 s', onclick: () => { state.sub = null; show(); } }, t('close')),
    h('b', {}, situation.name_ar)),
  ...rows.map((s) => h('div', { class: `rowline${s.active ? '' : ' off'}` },
    h('span', { class: 'latin small' }, `${s.lesson}.${s.position}`),
    h('div', { class: 'sent-text' }, s.text, h('div', { class: 'muted', style: 'direction:rtl;font-family:Cairo' }, s.translation)),
    h('span', { class: 'status' }, s.audio_ready ? t('admin_audio_ok') : t('admin_audio_missing')),
    h('button', { class: 'btn2 s', onclick: () => act(async () => { await api('POST', `/api/admin/sentences/${s.id}/active`, { active: !s.active }); show(); }) }, s.active ? t('admin_disable') : t('admin_enable')),
    h('button', { class: 'btn2 s', onclick: () => act(() => api('POST', `/api/admin/sentences/${s.id}/regenerate-audio`), t('admin_started')) }, t('admin_regen_audio')))));
  return [list];
}

async function content() {
  if (state.sub) return sentencesView(state.sub);
  const [sits, jobs] = await Promise.all([api('GET', '/api/admin/situations'), api('GET', '/api/admin/jobs')]);
  if (jobs.some((j) => j.status === 'running')) state.refresh = setTimeout(show, 5000);

  const form = {
    slug: h('input', { placeholder: 'slug', dir: 'ltr', 'aria-label': 'slug' }),
    ar: h('input', { placeholder: t('admin_name_ar'), 'aria-label': t('admin_name_ar') }),
    en: h('input', { placeholder: t('admin_name_en'), dir: 'ltr', 'aria-label': t('admin_name_en') }),
    icon: h('select', { 'aria-label': 'icon' }, SITUATION_ICONS.map((i) => h('option', { value: i }, i))),
  };
  const create = () => act(async () => {
    await api('POST', '/api/admin/situations', { slug: form.slug.value, name_ar: form.ar.value, name_en: form.en.value, icon: form.icon.value });
    show();
  }, t('admin_started'));

  return [
    h('div', { class: 'card full stack' }, h('b', {}, t('admin_new_situation')),
      h('div', { class: 'field' }, form.slug, form.ar, form.en, form.icon, h('button', { class: 'btn s', onclick: create }, t('admin_generate'))),
      h('div', { class: 'muted small' }, t('admin_new_note'))),
    h('div', { class: 'card full' }, ...sits.map((s) => h('div', { class: `rowline${s.active ? '' : ' off'}` },
      h('div', { class: 'grow' }, h('b', {}, s.name_ar), ' ', h('span', { class: 'muted latin small' }, `${s.slug} · ${s.sentences}`)),
      h('span', { class: 'status' }, s.active ? t('admin_published') : t('admin_draft')),
      s.audio_missing > 0 ? h('span', { class: 'status failed' }, `${t('admin_audio_missing')} ${s.audio_missing}`) : null,
      h('button', { class: 'btn2 s', onclick: () => { state.sub = s; show(); } }, t('admin_review')),
      h('button', { class: 'btn2 s', onclick: () => act(() => api('POST', `/api/admin/situations/${s.id}/generate`), t('admin_started')) }, t('admin_resume')),
      h('button', { class: 'btn2 s', onclick: () => act(() => api('POST', `/api/admin/situations/${s.id}/regenerate-audio`), t('admin_started')) }, t('admin_regen_audio')),
      h('button', { class: 'btn s', onclick: () => act(async () => { await api('POST', `/api/admin/situations/${s.id}/publish`, { active: !s.active }); show(); }) }, s.active ? t('admin_unpublish') : t('admin_publish'))))),
    h('div', { class: 'card full' }, h('b', {}, t('admin_jobs')),
      ...(jobs.length ? jobs.map((j) => h('div', { class: 'rowline' },
        h('span', { class: 'latin small' }, `#${j.id} ${j.kind}`), h('span', { class: `status ${j.status}` }, j.status),
        h('span', { class: 'muted small grow' }, j.detail))) : [h('div', { class: 'muted' }, t('admin_none'))]))];
}

const SECTIONS = { overview, users, payments, settings, content };

async function show() {
  clearTimeout(state.refresh);
  frame([h('div', { class: 'muted center' }, t('loading'))]);
  try {
    frame(await SECTIONS[state.tab]());
  } catch (e) {
    frame([h('div', { class: 'center-screen' }, t(e instanceof ApiError && e.status === 403 ? 'admin_denied' : 'error_generic'))]);
  }
}

async function start() {
  tg?.ready();
  tg?.expand();
  let me;
  try {
    me = await api('GET', '/api/me');
  } catch (e) {
    await loadLanguage('ar');
    root.replaceChildren(h('div', { class: 'center-screen' }, t(e instanceof ApiError && e.status === 401 ? 'error_auth' : 'error_generic')));
    return;
  }
  state.me = me;
  await loadLanguage(me.lang);
  show();
}

start();
