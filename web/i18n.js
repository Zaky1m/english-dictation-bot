let strings = {};
let list = null;

/** Languages from `i18n/languages.json`; the first one is the fallback. Fetched once. */
export function languages() {
  list ??= fetch('/i18n/languages.json').then((r) => r.json());
  return list;
}

export async function loadLanguage(lang) {
  const all = await languages();
  const language = all.find((l) => l.code === lang) ?? all[0];
  strings = await (await fetch(`/i18n/${language.code}.json`)).json();
  document.documentElement.lang = language.code;
  document.documentElement.dir = language.dir;
}

export function t(key, vars = {}) {
  let text = strings[key] ?? key;
  for (const [name, value] of Object.entries(vars)) text = text.replace(`{${name}}`, value);
  return text;
}
