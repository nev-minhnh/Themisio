// Themisio ↔ draw.io. draw.io runs in embed mode (proto=json) and talks to "its parent window"
// with postMessage; here the parent is Themisio, reached through the webview's IPC channel.
(function () {
  if (window.__themisio) return;
  var send = function (m) {
    try { window.ipc.postMessage(typeof m === 'string' ? m : JSON.stringify(m)); } catch (e) {}
  };
  // draw.io posts to window.opener || window.parent and accepts messages whose source is that
  // window: this stand-in is both.
  var host = { postMessage: function (m) { send(m); }, closed: false, focus: function () {} };
  try { Object.defineProperty(window, 'opener', { get: function () { return host; }, configurable: true }); } catch (e) {}
  window.__themisio = {
    deliver: function (json) {
      var e = new MessageEvent('message', { data: json, origin: location.origin });
      Object.defineProperty(e, 'source', { value: host });
      window.dispatchEvent(e);
    }
  };
  // Links open in the system browser, never inside the editor.
  window.open = function (url) {
    if (url) send({ event: 'themisio-open-url', url: String(url) });
    return null;
  };
  // Keys Themisio uses for itself (switching tabs and apps) go to it.
  window.addEventListener('keydown', function (e) {
    if (e.ctrlKey && (e.key === 'Tab' || (e.altKey && /^[1-9]$/.test(e.key)))) {
      send({ event: 'themisio-key', key: e.key, shift: e.shiftKey, alt: e.altKey });
    }
  }, true);
})();
