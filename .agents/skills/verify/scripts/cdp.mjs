#!/usr/bin/env node
// Background CDP driver for CodexBar's WebView2 windows. It never moves the OS cursor or takes focus.
// Pages are picked by their Tauri window label (main, settings, flyout, floatbar), never by list order.
//
//   node cdp.mjs <port> list                          -> JSON [{label,url,title}]
//   node cdp.mjs <port> ready [timeoutMs]             -> waits until a labelled page has rendered #root
//   node cdp.mjs <port> eval <label> <js>             -> JSON value of the (awaited) expression
//   node cdp.mjs <port> wait <label> <js> [timeoutMs] -> polls until the expression is truthy, prints it
//   node cdp.mjs <port> click <label> <css|js:expr>   -> dispatches a real mouse click at the element centre;
//                                                        "js:<expr>" picks the element with an expression
//   node cdp.mjs <port> invoke <label> <cmd> [json]   -> calls a Tauri command through the app bridge
//   node cdp.mjs <port> shot <label> <out.png>        -> CDP screenshot (layout evidence only; not native proof)
//
// Exit 0 on success; exit 1 with the reason on stderr.
import { writeFileSync } from "node:fs";

const [port, cmd, ...args] = process.argv.slice(2);
const base = "http://127.0.0.1:" + port;
const LABEL_JS =
  "(window.__TAURI_INTERNALS__?.metadata?.currentWindow?.label ?? window.__TAURI_INTERNALS__?.metadata?.currentWebview?.label ?? null)";

class Session {
  static open(wsUrl) {
    return new Promise((resolve, reject) => {
      const ws = new WebSocket(wsUrl);
      ws.onopen = () => resolve(new Session(ws));
      ws.onerror = () => reject(new Error("CDP websocket connect failed: " + wsUrl));
    });
  }
  constructor(ws) {
    this.ws = ws;
    this.next = 0;
    this.pending = new Map();
    ws.onmessage = (event) => {
      const msg = JSON.parse(event.data);
      const waiter = msg.id && this.pending.get(msg.id);
      if (!waiter) return;
      this.pending.delete(msg.id);
      if (msg.error) waiter.reject(new Error(msg.error.message));
      else waiter.resolve(msg.result);
    };
  }
  send(method, params = {}) {
    const id = ++this.next;
    this.ws.send(JSON.stringify({ id, method, params }));
    return new Promise((resolve, reject) => this.pending.set(id, { resolve, reject }));
  }
  async eval(expression) {
    const r = await this.send("Runtime.evaluate", { expression, awaitPromise: true, returnByValue: true });
    if (r.exceptionDetails) {
      throw new Error(r.exceptionDetails.exception?.description ?? r.exceptionDetails.text);
    }
    return r.result.value;
  }
  close() {
    this.ws.close();
  }
}

async function pages() {
  const res = await fetch(base + "/json/list");
  const all = (await res.json()).filter((t) => t.type === "page");
  const out = [];
  for (const p of all) {
    const s = await Session.open(p.webSocketDebuggerUrl);
    try {
      out.push({ label: await s.eval(LABEL_JS), url: p.url, title: p.title, ws: p.webSocketDebuggerUrl });
    } finally {
      s.close();
    }
  }
  return out;
}

async function openLabel(label) {
  const all = await pages();
  const hits = all.filter((p) => p.label === label);
  if (hits.length !== 1) {
    throw new Error('expected one page labelled "' + label + '", found ' + hits.length + "; labels: " + JSON.stringify(all.map((p) => p.label)));
  }
  return Session.open(hits[0].ws);
}

async function withLabel(label, fn) {
  const s = await openLabel(label);
  try {
    return await fn(s);
  } finally {
    s.close();
  }
}

async function poll(fn, timeoutMs, what) {
  const deadline = Date.now() + timeoutMs;
  let last;
  while (Date.now() < deadline) {
    try {
      const value = await fn();
      if (value) return value;
      last = "falsy: " + JSON.stringify(value);
    } catch (err) {
      last = err.message;
    }
    await new Promise((r) => setTimeout(r, 500));
  }
  throw new Error("timed out after " + timeoutMs + " ms waiting for " + what + " (last: " + last + ")");
}

const print = (value) => console.log(JSON.stringify(value, null, 2));

const commands = {
  async list() {
    print((await pages()).map(({ label, url, title }) => ({ label, url, title })));
  },
  async ready(timeoutMs = "60000") {
    const hit = await poll(async () => {
      for (const p of await pages()) {
        if (!p.label) continue;
        const s = await Session.open(p.ws);
        try {
          if (await s.eval("(document.getElementById('root')?.childElementCount ?? 0) > 0")) return p.label;
        } finally {
          s.close();
        }
      }
      return null;
    }, Number(timeoutMs), "a rendered CodexBar page");
    print({ ready: hit });
  },
  async eval(label, js) {
    print(await withLabel(label, (s) => s.eval(js)));
  },
  async wait(label, js, timeoutMs = "15000") {
    print(await poll(() => withLabel(label, (s) => s.eval(js)), Number(timeoutMs), js));
  },
  async click(label, target) {
    await withLabel(label, async (s) => {
      const find = target.startsWith("js:") ? target.slice(3) : "document.querySelector(" + JSON.stringify(target) + ")";
      const box = await s.eval(
        "(() => { const el = " + find + "; if (!el) return null;" +
          " el.scrollIntoView({ block: 'center' }); const r = el.getBoundingClientRect();" +
          " return { x: r.x + r.width / 2, y: r.y + r.height / 2, w: r.width, h: r.height }; })()",
      );
      if (!box || box.w === 0 || box.h === 0) throw new Error("no visible element for " + target);
      for (const type of ["mouseMoved", "mousePressed", "mouseReleased"]) {
        await s.send("Input.dispatchMouseEvent", {
          type, x: box.x, y: box.y, button: type === "mouseMoved" ? "none" : "left", clickCount: 1,
        });
      }
      print({ clicked: target, at: { x: box.x, y: box.y } });
    });
  },
  async invoke(label, command, json = "{}") {
    print(await withLabel(label, (s) =>
      s.eval("window.__TAURI_INTERNALS__.invoke(" + JSON.stringify(command) + ", " + json + ")")));
  },
  async shot(label, out) {
    await withLabel(label, async (s) => {
      const { data } = await s.send("Page.captureScreenshot", { format: "png" });
      writeFileSync(out, Buffer.from(data, "base64"));
      print({ wrote: out });
    });
  },
};

if (!port || !commands[cmd]) {
  console.error("usage: node cdp.mjs <port> list|ready|eval|wait|click|invoke|shot ...");
  process.exit(1);
}
try {
  await commands[cmd](...args);
} catch (err) {
  console.error(err.message);
  process.exit(1);
}
