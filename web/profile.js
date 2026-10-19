import { api } from './api.js';
import { h } from './dom.js';
import { icon } from './icons.js';
import { nav } from './nav.js';
import { t } from './i18n.js';

export async function showProfile(root, ctx) {
  const [me, stats] = await Promise.all([api('GET', '/api/me'), api('GET', '/api/stats')]);
  ctx.me = me;
  const stat = (value, label) => h('div', { class: 'card center' }, h('div', { class: 'big latin' }, value), h('div', { class: 'muted small' }, label));
  const initial = [...(me.name || '?')][0].toUpperCase();
  root.replaceChildren(
    h('div', { class: 'top' },
      h('h1', { class: 'title grow' }, t('nav_profile')),
      h('button', { class: 'iconbtn', 'aria-label': t('settings_title'), onclick: () => ctx.go('#/settings'), html: icon('sliders') })),
    h('div', { class: 'body scroll' },
      h('div', { class: 'row full' },
        h('div', { class: 'avatar' }, initial),
        h('div', { class: 'grow namecol' },
          h('div', { class: 'headline small-head' }, me.name),
          me.username ? h('div', { class: 'muted small', dir: 'ltr' }, `@${me.username}`) : null),
        h('span', { class: 'tag' }, me.pro ? 'Pro' : t('plan_free'))),
      h('div', { class: 'grid2' },
        stat(stats.streak, t('stat_streak')),
        stat(stats.sentences, t('stat_sentences')),
        stat(stats.words_learned, t('stat_words')),
        stat(`${stats.accuracy}%`, t('stat_accuracy'))),
      me.pro && me.pro_until ? h('div', { class: 'muted center small' }, t('pro_until', { date: me.pro_until })) : null,
      h('button', { class: 'btn2', onclick: () => ctx.go('#/words') }, h('span', { html: icon('bookmark') }), t('my_words')),
      me.pro ? null : h('button', { class: 'btn', onclick: () => ctx.go('#/pro') }, h('span', { html: icon('crown') }), t('go_pro_title'))),
    nav('profile', ctx));
}
