// What every site of the bench shares: asking the server, building the page,
// and the troubles a run can be given (a pop-up over the page, a sign-in that
// runs out). Nothing here reports what the agent did: the server keeps that,
// from what each page asks of it.

const api = {
  async get(site, what, params = {}) {
    const answer = await fetch(`/api/${site}/${what}?` + new URLSearchParams(params));
    if (answer.status === 401) {
      location.reload();
      throw new Error('signed out');
    }
    const data = await answer.json();
    if (!answer.ok) throw new Error(data.error || 'could not load');
    if (data && data.popup) showPopup(site);
    return data;
  },
  async post(site, action, body = {}) {
    const answer = await fetch(`/api/${site}/${action}`, { method: 'POST', headers: { 'Content-Type': 'application/json' }, body: JSON.stringify(body) });
    if (answer.status === 401) {
      location.reload();
      throw new Error('signed out');
    }
    const data = await answer.json();
    if (!answer.ok) throw new Error(data.error || 'something went wrong');
    return data;
  },
};

function el(tag, attrs = {}, ...children) {
  const node = document.createElement(tag);
  for (const [name, value] of Object.entries(attrs || {})) {
    if (name === 'on') for (const [event, fn] of Object.entries(value)) node.addEventListener(event, fn);
    else if (name === 'class') node.className = value;
    else if (name === 'html') node.innerHTML = value;
    else if (value === true) node.setAttribute(name, '');
    else if (value !== false && value != null) node.setAttribute(name, value);
  }
  for (const child of children.flat()) if (child != null && child !== false) node.append(child.nodeType ? child : document.createTextNode(String(child)));
  return node;
}

function clear(node) {
  node.textContent = '';
  return node;
}

function toast(text, bad = false) {
  const box = el('div', { class: bad ? 'toast bad' : 'toast', role: 'status' }, text);
  document.body.append(box);
  setTimeout(() => box.remove(), bad ? 6000 : 3000);
}

function loading(node, words = 'Loading…') {
  clear(node).append(el('div', { class: 'loading', 'aria-busy': 'true' }, words));
}

const POPUPS = {
  mail: ['Try the new Postbox', 'Our new inbox groups your mail by topic. Want to switch now?', 'Maybe later'],
  docs: ['What’s new in Pages', 'You can now add tables and checklists from the Insert menu.', 'Got it'],
  chat: ['Turn on notifications?', 'Get a ping when someone mentions you.', 'Not now'],
  shop: ['Get 10% off your first order', 'Join our newsletter for deals every week.', 'No thanks'],
  calendar: ['Calendar has a new look', 'Week view now shows travel time.', 'Close'],
  settings: ['Secure your account', 'Turn on two-step sign-in for extra safety.', 'Remind me later'],
};

function showPopup(site) {
  if (document.querySelector('.modal-back.popup')) return;
  const [title, text, dismiss] = POPUPS[site] || ['Notice', '', 'Close'];
  const back = el('div', { class: 'modal-back popup' });
  const shut = () => back.remove();
  back.append(el('div', { class: 'modal', role: 'dialog', 'aria-modal': 'true', 'aria-label': title },
    el('button', { class: 'x', 'aria-label': 'Close', on: { click: shut } }, '×'),
    el('h2', {}, title), el('p', {}, text),
    el('div', { class: 'row end' }, el('button', { on: { click: shut } }, dismiss), el('button', { class: 'primary', on: { click: shut } }, 'Yes, show me'))));
  document.body.append(back);
}

function cookieBanner() {
  if (sessionStorage.getItem('cookies-answered')) return;
  const bar = el('div', { class: 'cookies', role: 'region', 'aria-label': 'Cookie choices' },
    el('span', {}, 'We use cookies to keep you signed in and to measure how the shop is used.'),
    el('button', { on: { click: () => { sessionStorage.setItem('cookies-answered', '1'); bar.remove(); } } }, 'Reject all'),
    el('button', { class: 'primary', on: { click: () => { sessionStorage.setItem('cookies-answered', '1'); bar.remove(); } } }, 'Accept all'));
  document.body.append(bar);
}

function modal(title, body, actions) {
  const back = el('div', { class: 'modal-back' });
  const close = () => back.remove();
  back.append(el('div', { class: 'modal', role: 'dialog', 'aria-modal': 'true', 'aria-label': title },
    el('button', { class: 'x', 'aria-label': 'Close', on: { click: close } }, '×'),
    el('h2', {}, title), body,
    el('div', { class: 'row end' }, actions.map(([words, fn, primary]) => el('button', { class: primary ? 'primary' : '', on: { click: async () => { if ((await fn()) !== false) close(); } } }, words)))));
  document.body.append(back);
  return close;
}

const DAYS = ['Monday', 'Tuesday', 'Wednesday', 'Thursday', 'Friday', 'Saturday', 'Sunday'];

function day(base, n) {
  const d = new Date(base + 'T12:00:00');
  d.setDate(d.getDate() + n);
  return d;
}

function shortDate(iso) {
  const d = new Date(iso.length === 10 ? iso + 'T12:00:00' : iso);
  return d.toLocaleDateString('en-US', { month: 'short', day: 'numeric', year: 'numeric' });
}
