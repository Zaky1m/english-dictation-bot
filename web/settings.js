import { api } from './api.js';
import { h } from './dom.js';
import { icon } from './icons.js';
import { header } from './nav.js';
import { languages, loadLanguage, t } from './i18n.js';

export async function showSettings(root, ctx) {
  const me = await api('GET', '/api/me');
  ctx.me = me;
  const langs = await languages();

  async function setLang(lang) {
    if (lang === me.lang) return;
    await api('PATCH', '/api/me', { lang });
    await loadLanguage(lang);
    showSettings(root, ctx);
  }
  async function toggleReminders() {
    await api('PATCH', '/api/me', { reminders_on: !me.reminders_on });
    showSettings(root, ctx);
  }

  const langBtn = (l) => h('button', { class: l.code === me.lang ? 'btn' : 'btn2 dim', style: 'height:46px', lang: l.code, dir: l.dir, onclick: () => setLang(l.code) }, l.name);
  root.replaceChildren(
    header(t('settings_title'), ctx, { back: '#/profile' }),
    h('div', { class: 'body' },
      h('div', { class: 'card stack full' },
        h('b', {}, t('language')),
        h('div', { class: 'grid2' }, ...langs.map(langBtn)),
        h('div', { class: 'muted small' }, t('language_note'))),
      h('div', { class: 'card row full' },
        h('div', { class: 'grow' }, h('b', {}, t('reminders')), h('div', { class: 'muted small' }, t('reminders_text'))),
        h('button', { class: `switch${me.reminders_on ? ' on' : ''}`, role: 'switch', 'aria-checked': String(me.reminders_on), 'aria-label': t('reminders'), onclick: toggleReminders }, h('i'))),
      h('button', { class: 'card row full linkrow', onclick: () => ctx.go('#/pro') },
        h('b', { class: 'grow' }, t('subscription')), h('span', { class: 'flip', html: icon('arrow') }))));
}
