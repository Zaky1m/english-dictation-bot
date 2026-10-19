import { api } from './api.js';
import { h, toast } from './dom.js';
import { icon } from './icons.js';
import { nav } from './nav.js';
import { t } from './i18n.js';

// Horizontal offsets that make the lesson path wind.
const WIND = [0, -52, -72, -52, 0, 52];

export async function showMap(root, ctx) {
  root.replaceChildren(h('div', { class: 'center-screen muted' }, t('loading')));
  const [map, me] = await Promise.all([api('GET', '/api/map'), api('GET', '/api/me')]);
  ctx.me = me;
  const lang = me.lang;
  const current = map.flatMap((s) => s.lessons).find((l) => l.state === 'open');

  const open = (lesson) => (lesson.state === 'locked' ? toast(t('locked')) : ctx.go(`#/lesson/${lesson.id}`));
  const node = (lesson) => {
    const isCurrent = current && lesson.id === current.id;
    const size = isCurrent ? 72 : 60;
    const cls = lesson.state === 'done' ? 'node done' : lesson.state === 'locked' ? 'node lock' : 'node';
    const body = lesson.state === 'done' ? icon('check') : lesson.state === 'locked' ? icon('lock') : `${lesson.number}`;
    const wrap = h('button', {
      class: 'nodewrap',
      style: `transform:translateX(${WIND[(lesson.number - 1) % WIND.length]}px)`,
      'aria-label': `${lesson.number}`,
      onclick: () => open(lesson),
    }, h('div', { class: cls, style: `width:${size}px;height:${size}px;font-size:${isCurrent ? 24 : 20}px`, html: body }));
    if (isCurrent) wrap.append(h('div', { class: 'tag start' }, t('start')));
    return wrap;
  };

  const situations = map.map((s) => {
    const done = s.lessons.filter((l) => l.state === 'done').length;
    const doneLessons = s.lessons.find((l) => l.state !== 'done');
    return h('section', { class: 'situation' },
      h('div', { class: 'card sithead' },
        h('span', { html: icon(s.icon, 'big') }),
        h('div', { class: 'grow' },
          h('div', { class: 'sit' }, lang === 'ar' ? s.name_ar : s.name_en),
          h('div', { class: 'muted small' }, t('lesson_n_of', { n: doneLessons ? doneLessons.number : s.lessons.length, total: s.lessons.length }))),
        h('div', { class: 'bar', style: 'width:70px' }, h('i', { style: `width:${Math.round((done / s.lessons.length) * 100)}%` }))),
      h('div', { class: 'path' }, s.lessons.map(node)));
  });

  const shortcuts = h('div', { class: 'shortcuts' },
    h('button', { class: 'btn2', onclick: () => ctx.go('#/challenge') }, t('challenge_today')),
    me.review_due > 0 ? h('button', { class: 'btn2', onclick: () => ctx.go('#/review') }, t('review_n', { n: me.review_due })) : null);

  root.replaceChildren(
    h('div', { class: 'top' },
      h('h1', { class: 'title grow' }, t('nav_map')),
      h('div', { class: 'chip' }, h('span', { html: icon('flame') }), h('span', { class: 'latin' }, me.streak.current)),
      h('div', { class: 'chip latin' }, `${me.usage.started}/${me.usage.limit}`)),
    h('div', { class: 'body scroll' }, shortcuts, ...situations),
    nav('map', ctx));
  document.querySelector('.nodewrap .node:not(.done):not(.lock)')?.scrollIntoView({ block: 'center' });
}
