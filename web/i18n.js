let strings = {};
let list = [];

/** Languages from `i18n/languages.json`; the first one is the fallback. */
export async function languages() {
  if (!list.length) list = await (await fetch('/i18n/languages.json')).json();
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
