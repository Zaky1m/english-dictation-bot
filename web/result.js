import { api } from './api.js';
import { h } from './dom.js';
import { icon } from './icons.js';
import { t } from './i18n.js';

export async function showResult(root, lessonId, ctx) {
  const session = ctx.takeSession(`lesson:${lessonId}`);
  const [me, map] = await Promise.all([api('GET', '/api/me'), api('GET', '/api/map')]);
  ctx.me = me;
  const lessons = map.flatMap((s) => s.lessons);
  const at = lessons.findIndex((l) => l.id === lessonId);
  const next = lessons.slice(at + 1).find((l) => l.state !== 'locked');

  const stat = (value, label) => h('div', {}, h('div', { class: 'big latin' }, value), h('div', { class: 'muted small' }, label));
  const minutes = session ? Math.floor(session.ms / 60000) : 0;
  const seconds = session ? Math.floor((session.ms % 60000) / 1000) : 0;
  const stats = session
    ? h('div', { class: 'card stats3' },
      stat(session.score, t('points')),
      stat(`${Math.round((session.clean / session.solved) * 100)}%`, t('accuracy')),
      stat(`${minutes}:${String(seconds).padStart(2, '0')}`, t('time')))
    : null;

  root.replaceChildren(
    h('div', { class: 'top' }, h('h1', { class: 'title grow' }, t('lesson_done'))),
    h('div', { class: 'body' },
      h('div', { class: 'bigcheck', html: icon('check') }),
      h('div', { class: 'headline' }, t('great')),
      stats,
      ...(session?.badges ?? []).map((id) => h('div', { class: 'card row' }, h('span', { html: icon('star', 'big') }),
        h('div', { class: 'grow' }, h('b', {}, t('badge_new')), h('div', { class: 'muted small' }, t(`badge_${id}`))))),
      me.streak.current > 0
        ? h('div', { class: 'card row' }, h('span', { html: icon('flame', 'big') }),
          h('div', { class: 'grow' }, h('b', {}, t('streak_days', { n: me.streak.current }))))
        : null,
      h('div', { class: 'spacer' }),
      next ? h('button', { class: 'btn', onclick: () => ctx.go(`#/lesson/${next.id}`) }, t('next_lesson')) : null,
      h('button', { class: 'btn2', onclick: () => ctx.go('#/') }, t('back_to_map'))));
}
