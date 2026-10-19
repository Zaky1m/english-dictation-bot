import { h } from './dom.js';
import { icon } from './icons.js';
import { t } from './i18n.js';

/** Bottom tab bar. `active` is 'map' or 'profile'. */
export function nav(active, ctx) {
  const tab = (name, hash, ico) =>
    h('button', { class: `tab${active === name ? ' on' : ''}`, onclick: () => ctx.go(hash) },
      h('i', { html: icon(ico) }), t(`nav_${name}`));
  return h('nav', { class: 'nav' }, tab('map', '#/', 'pin'), tab('profile', '#/profile', 'user'));
}

/** Standard header with an optional back arrow and a title. */
export function header(title, ctx, { back = '#/', right = null } = {}) {
  return h('div', { class: 'top' },
    back ? h('button', { class: 'iconbtn', 'aria-label': t('close'), onclick: () => ctx.go(back), html: icon('close') }) : null,
    h('h1', { class: 'title grow' }, title),
    right);
}
