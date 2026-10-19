import { api, ApiError } from './api.js';

// Each adapter shows one rewarded ad and resolves 'reward' or 'skip'. It throws
// when the network has no ad or fails, so the next network gets a turn.
const SHOW_TIMEOUT_MS = 45000;
const scripts = new Map();

function loadScript(src, attrs = {}) {
  const key = src + JSON.stringify(attrs);
  if (!scripts.has(key)) {
    scripts.set(key, new Promise((resolve, reject) => {
      const el = document.createElement('script');
      el.src = src;
      el.async = true;
      for (const [name, value] of Object.entries(attrs)) el.setAttribute(name, value);
      el.onload = resolve;
      el.onerror = () => { scripts.delete(key); reject(new Error('sdk_load_failed')); };
      document.head.append(el);
    }));
  }
  return scripts.get(key);
}

function waitFor(check, timeoutMs = 8000) {
  const deadline = Date.now() + timeoutMs;
  return new Promise((resolve, reject) => {
    (function poll() {
      let value = null;
      try { value = check(); } catch { value = null; }
      if (value) return resolve(value);
      if (Date.now() > deadline) return reject(new Error('sdk_unavailable'));
      setTimeout(poll, 100);
    })();
  });
}

// A network promise that never settles would freeze the screen.
function withTimeout(promise, ms = SHOW_TIMEOUT_MS) {
  return Promise.race([promise, new Promise((_, reject) => setTimeout(() => reject(new Error('show_timeout')), ms))]);
}

const adapters = {
  async adsgram(p) {
    await loadScript('https://sad.adsgram.ai/js/sad.min.js');
    const sdk = await waitFor(() => window.Adsgram?.init && window.Adsgram);
    try {
      const result = await withTimeout(sdk.init({ blockId: String(p.block_id) }).show());
      return result?.done ? 'reward' : 'skip';
    } catch (e) {
      // Adsgram rejects with error=false when the viewer closed the ad early.
      if (e && e.error === false) return 'skip';
      throw e;
    }
  },

  async monetag(p) {
    const fn = `show_${p.zone_id}`;
    await loadScript('https://libtl.com/sdk.js', { 'data-zone': p.zone_id, 'data-sdk': fn });
    const show = await waitFor(() => typeof window[fn] === 'function' && window[fn]);
    // Resolves only after the rewarded ad was watched to the end.
    await withTimeout(Promise.resolve().then(() => show()));
    return 'reward';
  },

  async adexium(p) {
    const format = 'interstitial';
    await loadScript('https://cdn.tgads.space/assets/js/adexium-widget.min.js');
    const Widget = await waitFor(() => typeof window.AdexiumWidget === 'function' && window.AdexiumWidget);
    const widget = new Widget({ wid: p.widget_id, adFormat: format });
    return withTimeout(new Promise((resolve, reject) => {
      const done = (fn) => (arg) => { cleanup(); fn(arg); };
      const handlers = {
        adReceived: (ads) => widget.displayAd(ads, format),
        adPlaybackCompleted: done(() => resolve('reward')),
        // The close button appears only after playback, so closing first is a skip.
        adClosed: done(() => resolve('skip')),
        noAdFound: done(() => reject(new Error('no_ad'))),
        requestAdError: done(() => reject(new Error('request_failed'))),
      };
      function cleanup() { for (const [name, fn] of Object.entries(handlers)) widget.off?.(name, fn); }
      for (const [name, fn] of Object.entries(handlers)) widget.on(name, fn);
      Promise.resolve().then(() => widget.requestAd(format)).catch((e) => { cleanup(); reject(e); });
    }));
  },

  async telega(p) {
    await loadScript('https://inapp.telega.io/sdk/v1/sdk.js');
    const controller = await waitFor(() => window.TelegaIn?.AdsController?.create_miniapp && window.TelegaIn.AdsController);
    const ads = controller.create_miniapp({ token: p.token });
    const result = await withTimeout(Promise.resolve().then(() => ads.ad_show({ adBlockUuid: p.block_uuid })));
    return result?.done ? 'reward' : 'skip';
  },

  async gigapub(p) {
    const id = encodeURIComponent(p.project_id);
    try {
      await loadScript(`https://ad.gigapub.tech/script?id=${id}`);
      await waitFor(() => typeof window.showGiga === 'function', 15000);
    } catch {
      // GigaPub publishes a second host for when the first one is unreachable.
      await loadScript(`https://ru-ad.gigapub.tech/script?id=${id}`);
      await waitFor(() => typeof window.showGiga === 'function', 15000);
    }
    // showGiga walks several networks and resolves only after a completed view.
    await withTimeout(Promise.resolve().then(() => window.showGiga('main')), 120000);
    return 'reward';
  },
};

/**
 * Shows a rewarded ad from the first network that has one. Resolves true only
 * when an ad was watched to the end and the reward is on its way to the server.
 */
export async function showAd(providers = []) {
  for (const provider of providers) {
    const show = adapters[provider.name];
    if (!show) continue;
    let outcome;
    try {
      outcome = await show(provider);
    } catch {
      continue;
    }
    if (outcome !== 'reward') return false;
    // Adsgram tells our server itself; the others report through the app.
    if (provider.name !== 'adsgram') {
      try {
        await api('POST', '/api/ads/client-reward', { provider: provider.name });
      } catch {
        return false;
      }
    }
    return true;
  }
  return false;
}

/**
 * Adsgram's reward reaches our server a moment after the ad ends, so the claim
 * may need a few tries. Gives up on any error except "not credited yet".
 */
export async function withCredit(action) {
  for (let attempt = 0; attempt < 6; attempt += 1) {
    try {
      return await action();
    } catch (e) {
      if (!(e instanceof ApiError) || e.code !== 'ad_required' || attempt === 5) throw e;
      await new Promise((resolve) => setTimeout(resolve, 1000));
    }
  }
  return null;
}

export const claimExtra = () => withCredit(() => api('POST', '/api/ads/claim', { for: 'extra' }));
