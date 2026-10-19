import { api, ApiError } from './api.js';
import { h, toast } from './dom.js';
import { icon } from './icons.js';
import { t } from './i18n.js';

const tg = window.Telegram?.WebApp;
const LETTERS = /[^a-z]/g;

/** Buttons must not steal focus from the input, or the keyboard closes. */
const keepFocus = (e) => e.preventDefault();

/** Dictation screen for one lesson: hear, read the translation, type. */
export async function showExercise(root, lessonId, ctx) {
  const state = { stale: false };
  root.replaceChildren(h('div', { class: 'center-screen muted' }, t('loading')));
  ctx.onLeave(() => {
    state.stale = true;
    audio.pause();
  });
  const audio = new Audio();
  audio.preservesPitch = true;
  audio.webkitPreservesPitch = true;

  async function loadNext() {
    let attempt;
    try {
      attempt = await api('POST', `/api/lessons/${lessonId}/next`);
    } catch (e) {
      if (state.stale) return;
      const key = e instanceof ApiError && e.code === 'limit_reached' ? 'limit_reached'
        : e instanceof ApiError && e.code === 'locked' ? 'locked' : 'error_generic';
      root.replaceChildren(h('div', { class: 'center-screen' }, t(key)));
      return;
    }
    if (state.stale) return;
    if (attempt.done) return ctx.go(`#/result/${lessonId}`);
    render(attempt);
  }

  function render(attempt) {
    const words = attempt.tokens.map(() => '');
    const revealed = new Set();
    const results = []; // per word: { text, wrong: Set }
    let active = 0;
    let seq = 0;
    let controller = null;
    let timer = null;
    let played = false;
    let finished = false;

    const inputs = [];
    const tileRows = [];
    const sentence = h('div', { class: 'sent', dir: 'ltr' });

    attempt.tokens.forEach((token, i) => {
      const group = h('div', { class: 'word' });
      const tiles = [];
      const length = token.length;
      for (const ch of token.pattern) {
        const tile = h('div', { class: ch === '_' ? 'tile' : 'tile fix' }, ch === '_' ? '' : ch);
        if (ch === '_') tiles.push(tile);
        group.append(tile);
      }
      const input = h('input', {
        type: 'text',
        maxlength: length,
        autocomplete: 'off',
        autocorrect: 'off',
        autocapitalize: 'off',
        spellcheck: 'false',
        inputmode: 'text',
        enterkeyhint: 'next',
        'aria-label': `${i + 1}`,
      });
      input.addEventListener('focus', () => { active = i; paint(); });
      // Android keyboards sometimes skip beforeinput, so input sanitizes again.
      input.addEventListener('beforeinput', (e) => {
        if (e.data && /[^a-zA-Z]/.test(e.data)) {
          e.preventDefault();
          if (/\s/.test(e.data) && input.value !== '') focusWord(i + 1);
        }
      });
      input.addEventListener('input', () => {
        const clean = input.value.toLowerCase().replace(LETTERS, '').slice(0, length);
        if (clean !== input.value) input.value = clean;
        words[i] = clean;
        paint();
        scheduleCheck();
        if (clean.length === length) focusWord(i + 1);
      });
      input.addEventListener('keydown', (e) => {
        if (e.key === ' ') {
          e.preventDefault();
          // On an empty word a space is ignored, so typing "word word" after an
          // auto-advance does not skip a word.
          if (input.value !== '') focusWord(i + 1);
        } else if (e.key === 'Backspace' && input.value === '' && i > 0) {
          e.preventDefault();
          const prev = inputs[i - 1];
          prev.value = prev.value.slice(0, -1);
          prev.dispatchEvent(new Event('input'));
          focusWord(i - 1);
        }
      });
      group.append(input);
      inputs.push(input);
      tileRows.push(tiles);
      sentence.append(group);
    });

    function focusWord(i) {
      const target = inputs[Math.max(0, Math.min(i, inputs.length - 1))];
      target.focus();
    }

    function paint() {
      tileRows.forEach((tiles, i) => {
        const result = results[i];
        tiles.forEach((tile, k) => {
          const ch = words[i][k] ?? '';
          tile.textContent = ch;
          const checked = result && ch && result.text[k] === ch;
          tile.className = 'tile'
            + (revealed.has(i) ? ' rev' : checked && !result.wrong.has(k) ? ' ok' : '')
            + (checked && result.wrong.has(k) ? ' bad' : '')
            + (!finished && i === active && k === words[i].length ? ' cur' : '');
        });
      });
    }

    function fitTiles() {
      const longest = Math.max(...attempt.tokens.map((tok) => tok.pattern.length));
      const width = sentence.clientWidth || 320;
      const tile = Math.max(16, Math.min(26, Math.floor((width - 8) / longest) - 3));
      sentence.style.setProperty('--tile', `${tile}px`);
    }

    function scheduleCheck() {
      clearTimeout(timer);
      if (words.every((w) => w === '')) return;
      timer = setTimeout(runCheck, 500);
    }

    async function runCheck() {
      const mine = ++seq;
      controller?.abort();
      controller = new AbortController();
      let data;
      try {
        data = await api('POST', `/api/attempts/${attempt.attempt_id}/check`, { words: [...words] }, controller.signal);
      } catch (e) {
        if (e.name === 'AbortError' || state.stale) return;
        if (e instanceof ApiError && e.status === 429) timer = setTimeout(runCheck, 350);
        return;
      }
      if (mine !== seq || state.stale) return; // a newer check is in flight
      const before = attempt.tokens.map((_, i) => results[i]?.wrong.size ?? 0);
      data.words.forEach((w, i) => { results[i] = { text: words[i], wrong: new Set(w.wrong) }; });
      if (data.words.some((w, i) => w.wrong.length > before[i])) tg?.HapticFeedback?.notificationOccurred('error');
      paint();
      if (data.solved) solved(data);
    }

    function playSentence(rate) {
      audio.src = attempt.audio_url;
      audio.playbackRate = rate;
      audio.currentTime = 0;
      audio.play().catch(() => {});
      if (!played) {
        played = true;
        api('POST', `/api/attempts/${attempt.attempt_id}/play`).catch(() => {});
      }
    }

    async function reveal() {
      if (finished) return;
      // The current word, or the next one not yet revealed if it already was.
      const target = revealed.has(active) ? words.findIndex((_, i) => !revealed.has(i)) : active;
      if (target < 0) return;
      try {
        const data = await api('POST', `/api/attempts/${attempt.attempt_id}/reveal`, { word_index: target });
        words[target] = data.letters;
        inputs[target].value = data.letters;
        revealed.add(target);
        paint();
        clearTimeout(timer);
        runCheck();
      } catch (e) {
        toast(e instanceof ApiError && e.code === 'ad_required' ? t('reveal_needs_ad') : t('error_generic'));
      }
    }

    function solved(data) {
      finished = true;
      clearTimeout(timer);
      inputs.forEach((i) => { i.disabled = true; });
      tg?.HapticFeedback?.notificationOccurred('success');
      if (data.next_audio_url) {
        const preload = new Audio();
        preload.preload = 'auto';
        preload.src = data.next_audio_url;
      }
      showSolved(data);
    }

    function showSolved(data) {
      const popover = h('div', { class: 'popover', hidden: true });
      const chips = h('div', { class: 'chips', dir: 'ltr' });
      const texts = data.sentence.text.split(/\s+/);
      let selected = null;
      texts.forEach((word, idx) => {
        const meaning = data.meanings.find((m) => m.idx === idx);
        const chip = h('button', {
          class: 'latin',
          onclick: () => {
            selected?.classList.remove('sel');
            selected = chip;
            chip.classList.add('sel');
            const wordAudio = new Audio(meaning.audio_url);
            wordAudio.play().catch(() => {});
            popover.hidden = false;
            popover.replaceChildren(
              h('div', { class: 'row' },
                h('div', { class: 'word-en' }, word.replace(/[?.,!]+$/g, '')),
                h('button', { class: 'roundbtn', 'aria-label': 'play', onclick: () => new Audio(meaning.audio_url).play().catch(() => {}), html: icon('play') })),
              h('div', { style: 'font-size:18px;font-weight:700' }, meaning.meaning_ar),
              h('button', {
                class: 'btn', style: 'height:40px;font-size:14px',
                onclick: async () => {
                  try { await api('POST', `/api/words/${encodeURIComponent(meaning.word)}/save`); toast(t('word_saved')); }
                  catch { toast(t('error_generic')); }
                },
              }, h('span', { html: icon('bookmark') }), t('save_word')));
          },
        }, word);
        chips.append(chip);
      });
      const next = h('button', { class: 'btn', onclick: () => (data.lesson_complete ? ctx.go(`#/result/${lessonId}`) : loadNext()) }, t('next'));
      body.replaceChildren(
        h('div', { class: 'card prompt' }, h('div', { class: 'text', style: 'font-size:20px' }, attempt.translation)),
        chips,
        popover,
        h('div', { class: 'card scorecard' },
          h('div', {}, h('div', { class: 'muted', style: 'font-size:12px' }, t('score')), h('div', { class: 'big latin' }, `+${data.score.score}`)),
          h('div', {}, h('div', { class: 'muted', style: 'font-size:12px' }, t('combo')), h('div', { class: 'big latin' }, `x${data.score.combo}`))),
        h('div', { class: 'muted center', style: 'font-size:13px' }, t('tap_word_hint')),
        h('div', { class: 'spacer' }),
        next);
    }

    const total = attempt.total ?? 10;
    const index = attempt.index ?? 1;
    const body = h('div', { class: 'body' },
      h('div', { class: 'card prompt' },
        h('div', { class: 'label muted' }, t('type_in_english')),
        h('div', { class: 'text' }, attempt.translation)),
      h('div', { class: 'player' },
        h('button', { class: 'play', 'aria-label': 'play', onpointerdown: keepFocus, onclick: () => playSentence(1), html: icon('play') }),
        h('button', { class: 'pill', onpointerdown: keepFocus, onclick: () => playSentence(0.7) }, h('span', { html: icon('play') }), t('slower'), h('span', { class: 'latin' }, '0.7x'))),
      h('div', { class: 'muted', style: 'font-size:13px' }, t('words_count', { n: attempt.tokens.length })),
      sentence,
      h('button', { class: 'btn2 btn small', style: 'background:var(--card);color:var(--ink)', onpointerdown: keepFocus, onclick: reveal },
        ctx.me.ads_enabled && !ctx.me.pro ? t('reveal_needs_ad') : t('reveal_word')));
    root.replaceChildren(
      h('div', { class: 'top' },
        h('button', { class: 'iconbtn', 'aria-label': t('close'), onclick: () => ctx.go('#/'), html: icon('close') }),
        h('div', { class: 'bar grow' }, h('i', { style: `width:${Math.round(((index - 1) / total) * 100)}%` })),
        h('div', { class: 'chip latin' }, `${index} / ${total}`)),
      body);
    fitTiles();
    paint();
    inputs[0]?.focus({ preventScroll: true });
  }

  await loadNext();
}
