import { api, ApiError, leaveIfNotPublic } from '../api.js';
import { loadLanguage, t } from '../i18n.js';
import { h, toast } from '../dom.js';
import { SITUATION_ICONS, icon } from '../icons.js';

const tg = window.Telegram?.WebApp;
const root = document.getElementById('app');
const state = { tab: 'overview', me: null, refresh: null };
const TABS = ['overview', 'users', 'payments', 'settings', 'content'];
const TAB_ICONS = { overview: 'home', users: 'users', payments: 'star', settings: 'sliders', content: 'book' };

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
    h('div', { class: 'tabs', role: 'tablist' }, TABS.map((tab) => h('button', {
      class: state.tab === tab ? 'on' : '',
      role: 'tab',
      'aria-selected': String(state.tab === tab),
      onclick: () => { state.tab = tab; state.sub = null; show(); },
    }, h('span', { html: icon(TAB_ICONS[tab]) }), h('span', { class: 'label' }, t(`admin_tab_${tab}`))))),
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

const chip = (text, kind = '') => h('span', { class: `status ${kind}` }, text);
const item = (title, meta, chips, actions, extra = []) => h('div', { class: 'item' },
  h('div', { class: 'item-head' },
    h('div', { class: 'grow' }, title, meta ? h('div', { class: 'meta' }, meta) : null),
    chips.length ? h('div', { class: 'chips' }, ...chips) : null),
  ...extra,
  actions.length ? h('div', { class: 'actions' }, ...actions) : null);
const none = () => h('div', { class: 'muted' }, t('admin_none'));

async function users() {
  const results = h('div', { class: 'card full' });
  const input = h('input', { type: 'search', placeholder: t('admin_search_hint'), 'aria-label': t('admin_search_hint') });

  async function search() {
    const rows = await api('GET', `/api/admin/users?q=${encodeURIComponent(input.value)}`);
    results.replaceChildren(...(rows.length ? rows.map(userRow) : [none()]));
  }

  function userRow(u) {
    const days = h('input', { type: 'number', min: '1', max: '3650', inputmode: 'numeric', placeholder: t('admin_days'), 'aria-label': t('admin_days') });
    const grant = (plan, extra = {}) => act(async () => { await api('POST', `/api/admin/users/${u.id}/grant`, { plan, ...extra }); await search(); }, t('admin_done'));
    const revoke = () => act(async () => { await api('POST', `/api/admin/users/${u.id}/revoke`); await search(); }, t('admin_done'));
    return item(
      h('b', {}, u.name || '-'),
      `${u.username ? `@${u.username} · ` : ''}${u.id}`,
      [u.pro_until ? chip(`Pro · ${date(u.pro_until)}`, 'pro') : chip(t('plan_free'))],
      [
        h('button', { class: 'btn s', onclick: () => grant('month') }, t('plan_month')),
        h('button', { class: 'btn s', onclick: () => grant('year') }, t('plan_year')),
        h('div', { class: 'days' }, days, h('button', { class: 'btn2 s', onclick: () => grant('custom', { days: Number(days.value) }) }, t('admin_grant_days'))),
        u.pro_until ? h('button', { class: 'btn2 s warn', style: 'grid-column:1/-1', onclick: revoke }, t('admin_revoke')) : null,
      ].filter(Boolean));
  }

  input.addEventListener('keydown', (e) => { if (e.key === 'Enter') search(); });
  await search();
  return [h('div', { class: 'search' }, input, h('button', { class: 'btn s', onclick: search }, t('admin_search'))), results];
}

async function payments() {
  const data = await api('GET', '/api/admin/payments');
  return [
    h('div', { class: 'card full' }, h('h2', { class: 'section-title' }, t('admin_payments')),
      ...(data.payments.length ? data.payments.map((p) => item(
        h('b', {}, p.name), `${p.user_id} · ${p.payload}`,
        [chip(`${p.amount} ${p.currency}`, 'pro')], [], [h('div', { class: 'meta' }, date(p.created_at))])) : [none()])),
    h('div', { class: 'card full' }, h('h2', { class: 'section-title' }, t('admin_grants')),
      ...(data.grants.length ? data.grants.map((g) => item(
        h('b', {}, g.name), `${g.user_id} · ${date(g.starts_at)} - ${date(g.ends_at)}`,
        [chip(g.plan)], [])) : [none()]))];
}

async function settings() {
  const rows = await api('GET', '/api/admin/settings');
  const save = (key, value) => act(() => api('PUT', `/api/admin/settings/${key}`, { value }), t('admin_saved'));
  const list = h('div', { class: 'card full' });
  const filter = h('input', { type: 'search', dir: 'ltr', placeholder: t('admin_settings_filter'), 'aria-label': t('admin_settings_filter') });

  const rowsEls = rows.map(([key, value]) => {
    const name = h('div', { class: 'key' }, key);
    if (typeof value === 'boolean') {
      // A switch saves at once; the other kinds need an explicit save.
      let on = value;
      const toggle = h('button', { class: `switch${on ? ' on' : ''}`, role: 'switch', 'aria-checked': String(on), 'aria-label': key });
      toggle.append(h('i'));
      toggle.onclick = async () => {
        on = !on;
        toggle.classList.toggle('on', on);
        toggle.setAttribute('aria-checked', String(on));
        await save(key, on);
      };
      return h('div', { class: 'setting toggle', 'data-key': key }, name, toggle);
    }
    let control;
    let read;
    if (Array.isArray(value)) {
      const texts = value.some((x) => typeof x === 'string');
      control = h('input', { type: 'text', value: value.join(', '), dir: 'ltr' });
      const parts = () => control.value.split(',').map((x) => x.trim()).filter(Boolean);
      read = texts ? parts : () => parts().map(Number).filter((x) => !Number.isNaN(x));
    } else if (typeof value === 'number') {
      control = h('input', { type: 'number', step: 'any', inputmode: 'decimal', value: String(value), dir: 'ltr' });
      read = () => Number(control.value);
    } else {
      control = h('input', { type: 'text', value: String(value), dir: 'ltr' });
      read = () => control.value;
    }
    control.setAttribute('aria-label', key);
    return h('div', { class: 'setting', 'data-key': key }, name,
      h('div', { class: 'control' }, control, h('button', { class: 'btn s', onclick: () => save(key, read()) }, t('admin_save'))));
  });
  list.append(...rowsEls);
  filter.addEventListener('input', () => {
    const q = filter.value.trim().toLowerCase();
    for (const el of rowsEls) el.hidden = Boolean(q) && !el.dataset.key.includes(q);
  });
  return [filter, list];
}

async function sentencesView(situation) {
  const rows = await api('GET', `/api/admin/situations/${situation.id}/sentences`);
  const head = h('div', { class: 'item-head' },
    h('h2', { class: 'section-title grow' }, situation.name_ar),
    h('button', { class: 'btn2 s', style: 'width:auto', onclick: () => { state.sub = null; show(); } }, t('close')));
  return [h('div', { class: 'card full' }, head,
    ...rows.map((s) => {
      const el = item(
        h('div', { class: 'sent-text' }, s.text),
        `${s.lesson}.${s.position}`,
        [s.audio_ready ? chip(t('admin_audio_ok')) : chip(t('admin_audio_missing'), 'failed')],
        [
          h('button', { class: 'btn2 s', onclick: () => act(async () => { await api('POST', `/api/admin/sentences/${s.id}/active`, { active: !s.active }); show(); }) }, s.active ? t('admin_disable') : t('admin_enable')),
          h('button', { class: 'btn2 s', onclick: () => act(() => api('POST', `/api/admin/sentences/${s.id}/regenerate-audio`), t('admin_started')) }, t('admin_regen_audio')),
        ],
        [h('div', { class: 'sent-ar' }, s.translation)]);
      if (!s.active) el.classList.add('off');
      return el;
    }))];
}

async function content() {
  if (state.sub) return sentencesView(state.sub);
  const [sits, jobs] = await Promise.all([api('GET', '/api/admin/situations'), api('GET', '/api/admin/jobs')]);
  if (jobs.some((j) => j.status === 'running')) state.refresh = setTimeout(show, 5000);

  const form = {
    slug: h('input', { placeholder: 'slug', dir: 'ltr', 'aria-label': 'slug' }),
    icon: h('select', { 'aria-label': 'icon', dir: 'ltr' }, SITUATION_ICONS.map((i) => h('option', { value: i }, i))),
    ar: h('input', { class: 'wide', placeholder: t('admin_name_ar'), 'aria-label': t('admin_name_ar') }),
    en: h('input', { class: 'wide', placeholder: t('admin_name_en'), dir: 'ltr', 'aria-label': t('admin_name_en') }),
  };
  const create = () => act(async () => {
    await api('POST', '/api/admin/situations', { slug: form.slug.value, name_ar: form.ar.value, name_en: form.en.value, icon: form.icon.value });
    show();
  }, t('admin_started'));

  return [
    h('div', { class: 'card full stack' }, h('h2', { class: 'section-title' }, t('admin_new_situation')),
      h('div', { class: 'form' }, form.slug, form.icon, form.ar, form.en,
        h('button', { class: 'btn s wide', onclick: create }, t('admin_generate'))),
      h('div', { class: 'muted small' }, t('admin_new_note'))),
    h('div', { class: 'card full' }, ...sits.map((s) => {
      const el = item(
        h('b', {}, s.name_ar), `${s.slug} · ${s.sentences}`,
        [chip(s.active ? t('admin_published') : t('admin_draft'), s.active ? 'pro' : ''),
          s.audio_missing > 0 ? chip(`${t('admin_audio_missing')} ${s.audio_missing}`, 'failed') : null].filter(Boolean),
        [
          h('button', { class: 'btn2 s', onclick: () => { state.sub = s; show(); } }, t('admin_review')),
          h('button', { class: 'btn2 s', onclick: () => act(() => api('POST', `/api/admin/situations/${s.id}/generate`), t('admin_started')) }, t('admin_resume')),
          h('button', { class: 'btn2 s', onclick: () => act(() => api('POST', `/api/admin/situations/${s.id}/regenerate-audio`), t('admin_started')) }, t('admin_regen_audio')),
          h('button', { class: 'btn s', onclick: () => act(async () => { await api('POST', `/api/admin/situations/${s.id}/publish`, { active: !s.active }); show(); }) }, s.active ? t('admin_unpublish') : t('admin_publish')),
        ]);
      if (!s.active) el.classList.add('off');
      return el;
    })),
    h('div', { class: 'card full' }, h('h2', { class: 'section-title' }, t('admin_jobs')),
      ...(jobs.length ? jobs.map((j) => item(
        h('b', { class: 'latin' }, `#${j.id} ${j.kind}`), j.detail || null,
        [chip(j.status, j.status)], [])) : [none()]))];
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
    if (await leaveIfNotPublic(e)) return;
    await loadLanguage(tg?.initDataUnsafe?.user?.language_code);
    root.replaceChildren(h('div', { class: 'center-screen' }, t(e instanceof ApiError && e.status === 401 ? 'error_auth' : 'error_generic')));
    return;
  }
  state.me = me;
  await loadLanguage(me.lang);
  show();
}

start();
