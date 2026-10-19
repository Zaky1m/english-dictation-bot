import { api } from './api.js';
import { h } from './dom.js';
import { nav } from './nav.js';
import { t } from './i18n.js';

export async function showLeague(root, ctx) {
  const lg = await api('GET', '/api/league');
  const days = Math.max(0, Math.ceil((new Date(lg.week_end).getTime() - Date.now()) / 86400000));
  const total = lg.members.length;
  const rows = [];
  lg.members.forEach((m, i) => {
    if (i === lg.promote_top && total > lg.promote_top) rows.push(h('div', { class: 'zone up' }, t('league_zone_up')));
    if (total > lg.promote_top + lg.demote_bottom && i === total - lg.demote_bottom) rows.push(h('div', { class: 'zone down' }, t('league_zone_down')));
    rows.push(h('div', { class: `item${m.me ? ' me' : ''}` },
      h('span', { class: 'rank latin' }, m.rank),
      h('span', { class: `grow nm${m.pro ? ' pro' : ''}` }, m.name, m.pro ? h('span', { class: 'tag', style: 'margin-inline-start:6px;height:20px;font-size:11px;padding:0 8px' }, 'Pro') : null),
      h('span', { class: 'latin pts' }, m.points)));
  });
  root.replaceChildren(
    h('div', { class: 'top' },
      h('h1', { class: 'title grow' }, t('nav_league')),
      h('div', { class: 'chip' }, t('league_ends', { n: days }))),
    h('div', { class: 'body scroll' },
      h('div', { class: 'card center full' },
        h('div', { class: 'headline' }, t(`tier_${lg.tier}`)),
        h('div', { class: 'muted small' }, t('league_rule', { up: lg.promote_top, down: lg.demote_bottom }))),
      lg.joined
        ? h('div', { class: 'card list full' }, ...rows)
        : h('div', { class: 'muted center' }, t('league_empty'))),
    nav('league', ctx));
}
