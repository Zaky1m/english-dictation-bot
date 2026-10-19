import { api } from './api.js';
import { h } from './dom.js';
import { icon } from './icons.js';
import { header } from './nav.js';
import { t } from './i18n.js';

export async function showWords(root, ctx) {
  const words = await api('GET', '/api/words');
  const rows = words.map((w) => h('div', { class: 'item' },
    h('button', { class: 'roundbtn', 'aria-label': 'play', onclick: () => new Audio(w.audio_url).play().catch(() => {}), html: icon('play') }),
    h('div', { class: 'grow' }, h('div', { class: 'latin wordname' }, w.word), h('div', { class: 'muted' }, w.meaning_ar ?? ''))));
  root.replaceChildren(
    header(t('my_words'), ctx, { back: '#/profile' }),
    h('div', { class: 'body' }, words.length
      ? h('div', { class: 'card list' }, ...rows)
      : h('div', { class: 'muted center' }, t('words_empty'))));
}
