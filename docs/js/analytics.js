(function () {
  'use strict';

  // Skip localhost/dev environments
  if (
    location.hostname === 'localhost' ||
    location.hostname === '127.0.0.1' ||
    location.hostname === ''
  ) return;

  // Get or create a persistent visitor ID stored in localStorage
  var vid;
  try {
    vid = localStorage.getItem('_ah_vid');
    if (!vid) {
      vid =
        typeof crypto !== 'undefined' && typeof crypto.randomUUID === 'function'
          ? crypto.randomUUID()
          : Math.random().toString(36).slice(2) + Date.now().toString(36);
      localStorage.setItem('_ah_vid', vid);
    }
  } catch (e) {
    // Private browsing / storage blocked — use ephemeral ID
    vid = Math.random().toString(36).slice(2);
  }

  // Fire the page view beacon (analytics must never break the page)
  try {
    fetch('https://auth.asymptai.com/api/track', {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({
        page: location.pathname,
        referrer: document.referrer || '',
        visitor_id: vid,
      }),
      keepalive: true,
    }).catch(function () {});
  } catch (e) {}
})();
