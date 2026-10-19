import { api } from './api.js';
import { h } from './dom.js';
import { icon } from './icons.js';
import { nav } from './nav.js';
import { t } from './i18n.js';

export async function showLimit(root, ctx) {
  const me = await api('GET', '/api/me');
  ctx.me = me;
  const extra = me.usage.ad_unlocks_left > 0
    ? h('div', { class: 'card stack' },
      h('div', { class: 'row' }, h('span', { html: icon('video', 'big') }),
        h('div', { class: 'grow' }, h('b', {}, t('ad_extra_title')), h('div', { class: 'muted small' }, t('ad_extra_text')))),
      h('button', { class: 'btn', onclick: () => ctx.watchAd?.() }, t('watch_ad')))
    : null;
  root.replaceChildren(
    h('div', { class: 'top' }, h('h1', { class: 'title grow' }, t('nav_map'))),
    h('div', { class: 'body' },
      h('div', { class: 'bigicon', html: icon('clock') }),
      h('div', { class: 'headline' }, t('limit_reached')),
      h('div', { class: 'muted center' }, t('limit_text', { n: me.usage.started })),
      extra,
      h('div', { class: 'card stack' },
        h('div', { class: 'row' }, h('span', { html: icon('crown', 'big') }),
          h('div', { class: 'grow' }, h('b', {}, t('go_pro_title')), h('div', { class: 'muted small' }, t('go_pro_text')))),
        h('button', { class: 'btn dark', onclick: () => ctx.go('#/pro') }, t('learn_more'))),
      me.review_due > 0 ? h('button', { class: 'btn2', onclick: () => ctx.go('#/review') }, t('review_n', { n: me.review_due })) : null),
    nav('map', ctx));
}
