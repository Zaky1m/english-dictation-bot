import { api, ApiError } from './api.js';

const SDK = 'https://sad.adsgram.ai/js/sad.min.js';
let loading = null;

function loadSdk() {
  loading ??= new Promise((resolve, reject) => {
    if (window.Adsgram) return resolve();
    const script = document.createElement('script');
    script.src = SDK;
    script.onload = resolve;
    script.onerror = () => { loading = null; reject(new Error('sdk')); };
    document.head.append(script);
  });
  return loading;
}

/** Shows a rewarded ad. Resolves true only when the user watched it to the end. */
export async function showAd(blockId) {
  try {
    await loadSdk();
    const result = await window.Adsgram.init({ blockId: String(blockId) }).show();
    return result?.done === true;
  } catch {
    return false;
  }
}

/**
 * The reward reaches our server from Adsgram, a moment after the ad ends, so the
 * claim may need a few tries. Gives up on any error except "not credited yet".
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
