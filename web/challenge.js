import { api } from './api.js';
import { h } from './dom.js';
import { header, nav } from './nav.js';
import { t } from './i18n.js';

export async function showChallenge(root, ctx) {
  const today = await api('GET', '/api/challenge/today').catch(() => null);
  const finished = today && today.solved >= today.total;
  root.replaceChildren(
    header(t('challenge_title'), ctx),
    h('div', { class: 'body' },
      today
        ? h('div', { class: 'card stack center' },
          h('div', { class: 'headline' }, t('challenge_title')),
          h('div', { class: 'muted' }, t('challenge_text')),
          h('div', { class: 'bar' }, h('i', { style: `width:${Math.round((today.solved / today.total) * 100)}%` })),
          h('div', { class: 'latin big' }, `${today.solved} / ${today.total}`),
          today.solved > 0 ? h('div', { class: 'muted' }, t('challenge_score', { n: today.score })) : null)
        : h('div', { class: 'muted center' }, t('error_generic')),
      today ? h('button', { class: 'btn', disabled: finished, onclick: () => ctx.go('#/challenge/play') },
        finished ? t('challenge_done') : today.solved > 0 ? t('challenge_continue') : t('challenge_start')) : null),
    nav('map', ctx));
}
