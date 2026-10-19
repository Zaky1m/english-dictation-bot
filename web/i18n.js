let strings = {};

export async function loadLanguage(lang) {
  const code = lang === 'en' ? 'en' : 'ar';
  strings = await (await fetch(`/i18n/${code}.json`)).json();
  document.documentElement.lang = code;
  document.documentElement.dir = code === 'en' ? 'ltr' : 'rtl';
}

export function t(key, vars = {}) {
  let text = strings[key] ?? key;
  for (const [name, value] of Object.entries(vars)) text = text.replace(`{${name}}`, value);
  return text;
}
