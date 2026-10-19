import { api } from './api.js';
import { h } from './dom.js';
import { icon } from './icons.js';
import { nav } from './nav.js';
import { t } from './i18n.js';

const BADGE_ICON = {
  first_lesson: 'check', sentences_10: 'book', sentences_100: 'book', sentences_500: 'book', sentences_1000: 'book',
  streak_3: 'flame', streak_7: 'flame', streak_30: 'flame', streak_100: 'flame',
  perfect_lesson: 'star', fast_typist: 'bolt', first_promotion: 'trophy', situation_completed: 'pin',
};

function badgesCard(data) {
  const earned = new Set(data.earned.map((b) => b.badge));
  return h('div', { class: 'card stack full' },
    h('b', {}, t('badges_title')),
    h('div', { class: 'badges' }, data.all.map((id) =>
      h('div', { class: `badge${earned.has(id) ? ' on' : ''}`, title: t(`badge_${id}`) },
        h('div', { class: 'bubble', html: icon(earned.has(id) ? BADGE_ICON[id] : 'lock') }),
        h('div', { class: 'small' }, t(`badge_${id}`))))));
}

function chartCard(pro, ctx) {
  const max = Math.max(1, ...pro.days.map((d) => d.sentences));
  return h('div', { class: 'card stack full' },
    h('b', {}, t('chart_title')),
    h('div', { class: 'bars', dir: 'ltr' }, pro.days.map((d) =>
      h('div', { class: 'barcol', title: `${d.date}: ${d.sentences}` },
        h('i', { style: `height:${Math.round((d.sentences / max) * 100)}%` })))),
    pro.letters.length ? h('div', {}, h('div', { class: 'muted small' }, t('missed_letters')),
      h('div', { class: 'pills latin', dir: 'ltr' }, pro.letters.map(([l, n]) => h('span', { class: 'pill dim' }, `${l.toUpperCase()} ${n}`)))) : null,
    pro.words.length ? h('div', {}, h('div', { class: 'muted small' }, t('missed_words')),
      h('div', { class: 'pills latin', dir: 'ltr' }, pro.words.map(([w, n]) => h('span', { class: 'pill dim' }, `${w} ${n}`)))) : null);
}

export async function showProfile(root, ctx) {
  const [me, stats, badges] = await Promise.all([api('GET', '/api/me'), api('GET', '/api/stats'), api('GET', '/api/badges')]);
  const pro = me.pro ? await api('GET', '/api/stats/pro') : null;
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
          me.username ? h('div', { class: 'muted small' }, h('span', { dir: 'ltr' }, `@${me.username}`)) : null),
        h('span', { class: 'tag' }, me.pro ? 'Pro' : t('plan_free'))),
      h('div', { class: 'grid2' },
        stat(stats.streak, t('stat_streak')),
        stat(stats.sentences, t('stat_sentences')),
        stat(stats.words_learned, t('stat_words')),
        stat(`${stats.accuracy}%`, t('stat_accuracy'))),
      me.pro && me.pro_until ? h('div', { class: 'muted center small' }, t('pro_until', { date: me.pro_until })) : null,
      pro
        ? chartCard(pro, ctx)
        : h('button', { class: 'card stack full locked', onclick: () => ctx.go('#/pro') },
          h('div', { class: 'row' }, h('b', { class: 'grow' }, t('chart_title')), h('span', { class: 'tag' }, 'Pro')),
          h('div', { class: 'muted small' }, t('chart_pro_text'))),
      badgesCard(badges),
      h('button', { class: 'btn2', onclick: () => ctx.go('#/words') }, h('span', { html: icon('bookmark') }), t('my_words')),
      me.pro ? null : h('button', { class: 'btn', onclick: () => ctx.go('#/pro') }, h('span', { html: icon('crown') }), t('go_pro_title'))),
    nav('profile', ctx));
}
