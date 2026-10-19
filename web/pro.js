import { api } from './api.js';
import { h, toast } from './dom.js';
import { icon } from './icons.js';
import { header } from './nav.js';
import { t } from './i18n.js';

const tg = window.Telegram?.WebApp;

export async function showPro(root, ctx) {
  const me = await api('GET', '/api/me');
  ctx.me = me;
  const p = me.pricing;
  let plan = 'year';

  const price = (egp, usd) => h('div', {}, h('div', { class: 'latin big' }, `${egp} EGP`), h('div', { class: 'muted small latin' }, `${usd} USD`));
  const planCard = (id, egp, usd) => {
    const card = h('button', { class: `card plan${plan === id ? ' sel' : ''}`, onclick: () => { plan = id; render(); } },
      h('b', {}, t(`plan_${id}`)), price(egp, usd));
    return card;
  };

  async function payStars() {
    try {
      const { url } = await api('POST', `/api/pay/stars/${plan}`);
      if (tg?.openInvoice) {
        tg.openInvoice(url, async (status) => {
          if (status === 'paid') {
            toast(t('pro_active'));
            showPro(root, ctx);
          }
        });
      } else {
        window.open(url, '_blank');
      }
    } catch {
      toast(t('error_generic'));
    }
  }

  function manual(methodKey) {
    const text = t('manual_message', { plan: t(`plan_${plan}`), method: t(methodKey), id: me.id });
    const link = `https://t.me/${me.admin_username}?text=${encodeURIComponent(text)}`;
    if (tg?.openTelegramLink) tg.openTelegramLink(link);
    else window.open(link, '_blank');
  }

  function render() {
    const stars = plan === 'year' ? p.stars_year : p.stars_month;
    const methods = me.admin_username
      ? ['pay_vodafone', 'pay_instapay', 'pay_usdt'].map((key) =>
        h('button', { class: 'btn2 spread', onclick: () => manual(key) }, t(key), h('span', { class: 'flip', html: icon('arrow') })))
      : [];
    root.replaceChildren(
      header('Pro', ctx, { back: '#/profile' }),
      h('div', { class: 'body scroll' },
        h('div', { class: 'card stack full benefits' },
          ['benefit_1', 'benefit_2', 'benefit_3', 'benefit_4', 'benefit_5'].map((k) =>
            h('div', { class: 'row' }, h('span', { html: icon('check') }), t(k)))),
        h('div', { class: 'grid2 full' }, planCard('month', p.month_egp, p.month_usd), planCard('year', p.year_egp, p.year_usd)),
        h('b', { class: 'full' }, t('pay_method')),
        h('button', { class: 'btn spread', onclick: payStars },
          h('span', { class: 'row' }, h('span', { html: icon('star') }), t('pay_stars')), h('span', { class: 'latin' }, stars)),
        ...methods,
        me.admin_username ? h('div', { class: 'muted center small' }, t('pay_manual_note')) : null));
  }
  render();
}
