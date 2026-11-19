const GRACE = 60_000;
const HOST_TIMEOUT = 15_000;
const LIFETIME = 4 * 60 * 60_000;
const PAGE = 90_000; // below the DO KV value size limit
const MAX_PROJECT = 1024 * 1024 * 2; // encoded checkpoint
const MAX_ASSET = 6 * 1024 * 1024 * 2; // hex encoded binary (fits Android sender queue)
const ROOM_ASSETS = 32 * 1024 * 1024 * 2;
const CODE = /^[A-F0-9]{10}$/;
const HEX = /[^a-f0-9]/;
const validHex = (s, max) => typeof s === 'string' && s.length > 0 && s.length <= max && !(s.length % 2) && !HEX.test(s);
const secret = () => crypto.randomUUID().replaceAll('-', '') + crypto.randomUUID().replaceAll('-', '');
const answer = (status, text) => new Response(text, {status});
const tell = (ws, msg) => { try { ws.send(JSON.stringify(msg)); } catch { /* a closing socket is not a failed commit */ } };

export default {
  fetch(request, env) {
    const url = new URL(request.url);
    if (request.headers.get('Upgrade')?.toLowerCase() !== 'websocket') {
      if (url.pathname === '/health') return answer(200, 'Honeydew Rooms v1');
      if (url.pathname === '/join' && CODE.test(url.searchParams.get('code') || '')) {
        return answer(200, `Open Honeydew → Collab. Room code: ${url.searchParams.get('code')}`);
      }
      return answer(404, 'Not found');
    }
    const create = url.pathname === '/rooms/new';
    const code = create ? secret().slice(0, 10).toUpperCase() : url.pathname.slice(7);
    if (!create && (!url.pathname.startsWith('/rooms/') || !CODE.test(code))) return answer(404, 'Invalid room');
    url.pathname = `/internal/${code}/${create ? 'create' : 'join'}`;
    return env.ROOMS.get(env.ROOMS.idFromName(code)).fetch(new Request(url, request));
  },
};

export class Room {
  constructor(ctx) {
    this.ctx = ctx;
    this.storage = ctx.storage;
    this.state = null;
    ctx.blockConcurrencyWhile(async () => { this.state = await this.storage.get('room') || null; });
  }
  socket(role) {
    return this.ctx.getWebSockets(role).find(ws => ws.readyState === 1);
  }
  others(ws, msg) {
    for (const peer of this.ctx.getWebSockets()) if (peer !== ws) tell(peer, msg);
  }
  async schedule() {
    if (!this.state) return;
    const dates = this.state.solo ? [this.state.end] : [this.state.end, this.state.host?.until, this.state.guest?.until].filter(Boolean);
    await this.storage.setAlarm(Math.min(...dates));
  }
  async persist() {
    await this.storage.put('room', this.state);
    await this.schedule();
  }
  async blob(key) {
    const values = await this.storage.list({prefix: `${key}/`});
    return [...values.values()].join('');
  }
  async put(key, value, metadata) {
    await this.storage.transaction(async (txn) => {
      const old = await txn.list({prefix: `${key}/`});
      if (old.size) await txn.delete([...old.keys()]);
      for (let i = 0; i < value.length; i += PAGE) {
        await txn.put(`${key}/${String(i).padStart(9, '0')}`, value.slice(i, i + PAGE));
      }
      await txn.put('room', metadata);
    });
  }
  async sync(ws) {
    if (!this.state.rev) return;
    tell(ws, {type: 'state', rev: this.state.rev, checkpoint: await this.blob('project')});
  }
  catalog(ws) {
    tell(ws, {type: 'catalog', paths: Object.keys(this.state.assets)});
  }
  async getAsset(ws, path) {
    if (typeof path !== 'string' || !Object.hasOwn(this.state.assets, path)) return;
    const {key, hash} = this.state.assets[path];
    tell(ws, {type: 'asset', path, hash, data: await this.blob(`asset/${key}`)});
  }
  async finish() {
    for (const ws of this.ctx.getWebSockets()) {
      const role = ws.deserializeAttachment().role;
      tell(ws, {type: role === 'guest' ? 'solo' : 'ended'});
      ws.close(1000, 'Room ended');
    }
    this.state = null;
    await this.storage.deleteAll();
    await this.storage.deleteAlarm();
  }
  async promote(now) {
    const s = this.state;
    if (!s?.guest) return this.finish();
    s.solo = true;
    s.end = s.guest.until > now ? s.guest.until : now + GRACE;
    s.host.until = 0;
    s.lock = null;
    await this.persist();
    const guest = this.socket('guest');
    if (guest) {
      this.catalog(guest);
      await this.sync(guest);
      tell(guest, {type: 'solo'});
    }
    const host = this.socket('host');
    if (host) { tell(host, {type: 'ended'}); host.close(1000, 'Room ended'); }
  }
  async expire(now) {
    const s = this.state;
    if (!s) return;
    if (s.end <= now) {
      await this.finish();
    } else if (!s.solo && s.host.until && s.host.until <= now) {
      await this.promote(now);
    } else if (!s.solo && s.guest?.until && s.guest.until <= now) {
      s.guest = null;
      await this.persist();
      const host = this.socket('host');
      if (host) tell(host, {type: 'presence', name: null});
    }
  }
  async alarm() { await this.ctx.blockConcurrencyWhile(() => this.expire(Date.now())); }
  fetch(request) {
    return this.ctx.blockConcurrencyWhile(() => this.admit(request));
  }
  async admit(request) {
    const url = new URL(request.url);
    const [, , code, action] = url.pathname.split('/');
    const name = (url.searchParams.get('name') || '').trim();
    const token = request.headers.get('Authorization')?.replace(/^Bearer /, '');
    if (!name || name.length > 32) return answer(400, 'Display name must have 1–32 characters');
    const now = Date.now();
    await this.expire(now);
    let s = this.state;
    let role;
    if (action === 'create') {
      if (s) return answer(409, 'Try creating again');
      role = 'host';
      s = {code, rev: 0, end: now + LIFETIME, host: null, guest: null, assets: {}, bytes: 0, lock: null};
    } else {
      if (!s) return answer(404, 'Room not found or expired');
      role = token && token === s.host.token ? 'host' : 'guest';
      if (s.solo && (role !== 'guest' || !token || token !== s.guest?.token)) return answer(410, 'Room ended');
      if (s[role] && (!token || token !== s[role].token || this.socket(role))) return answer(409, 'Room seat already taken');
      if (token && !s[role]) return answer(410, 'Reconnect window expired');
      if (!s.rev) return answer(425, 'Host project is not ready yet');
    }
    s[role] = {name, token: s[role]?.token || secret(), until: 0};
    this.state = s;
    // Accept before awaiting persistence so another admission sees the occupied seat immediately.
    const pair = new WebSocketPair();
    const ws = pair[1];
    ws.serializeAttachment({role, token: s[role].token, start: now, count: 0});
    this.ctx.acceptWebSocket(ws, [role]);
    await this.persist();
    tell(ws, {type: 'welcome', role, code, token: s[role].token});
    this.catalog(ws);
    await this.sync(ws);
    if (s.solo) {
      // The guest downloads missing samples before accepting the solo checkpoint.
      tell(ws, {type: 'solo'});
    } else {
      const other = this.socket(role === 'host' ? 'guest' : 'host');
      if (other) {
        tell(ws, {type: 'presence', name: s[other.deserializeAttachment().role].name});
        tell(other, {type: 'presence', name});
      }
    }
    return new Response(null, {status: 101, webSocket: pair[0]});
  }
  async webSocketMessage(ws, wire) {
    // Serialize explicitly across async storage and digest calls, including check + commit.
    await this.ctx.blockConcurrencyWhile(async () => this.message(ws, wire));
  }
  async message(ws, wire) {
    const s = this.state;
    const a = ws.deserializeAttachment();
    if (!s || s[a.role]?.token !== a.token) return;
    const now = Date.now();
    if (now - a.start >= 1_000) { a.start = now; a.count = 0; }
    if (++a.count > 24) { ws.close(1008, 'Message rate exceeded'); return; }
    ws.serializeAttachment(a);
    if (typeof wire !== 'string' || wire.length > MAX_ASSET + 2048) { ws.close(1009, 'Message too large'); return; }
    let msg;
    try { msg = JSON.parse(wire); } catch { return; }
    if (!msg || typeof msg !== 'object') return;
    if (s.solo && !['sync', 'get_asset', 'leave'].includes(msg.type)) return;
    const fail = message => tell(ws, {type: 'error', message});
    switch (msg.type) {
      case 'commit': {
        if (!validHex(msg.checkpoint, MAX_PROJECT)) return fail('Checkpoint exceeds the 1 MiB room limit');
        if (s.rev !== msg.base) return this.sync(ws);
        if (s.lock?.until > now && s.lock?.role !== a.role) {
          // A conservative clip-level fence: do not commit a whole checkpoint while the other device is dragging.
          return tell(ws, {type: 'busy'});
        }
        const next = {...s, rev: s.rev + 1};
        await this.put('project', msg.checkpoint, next);
        this.state = next;
        tell(ws, {type: 'ack', rev: next.rev});
        this.others(ws, {type: 'state', checkpoint: msg.checkpoint, rev: next.rev});
        break;
      }
      case 'sync': this.catalog(ws); await this.sync(ws); break;
      case 'get_asset': await this.getAsset(ws, msg.path); break;
      case 'drag': {
        const {clip} = msg;
        if (clip !== null && (!Array.isArray(clip) || clip.length !== 3 || clip.some(n => !Number.isSafeInteger(n)))) return;
        if (s.lock?.until > now && s.lock.role !== a.role) return tell(ws, {type: 'drag', clip: s.lock.clip, name: s[s.lock.role]?.name});
        s.lock = clip ? {role: a.role, clip, until: now + 4_000} : null;
        this.others(ws, {type: 'drag', clip, name: s[a.role].name});
        break;
      }
      case 'play': if (typeof msg.playing === 'boolean') this.others(ws, {type: 'play', playing: msg.playing}); break;
      case 'solo': if (Array.isArray(msg.channels) && msg.channels.length <= 256 && msg.channels.every(Number.isSafeInteger)) this.others(ws, {type: 'solo_mode', channels: msg.channels}); break;
      case 'asset': {
        const {path, data, hash} = msg;
        if (typeof path !== 'string' || path.length > 512 || !path.startsWith('My ') || path.includes('\\') || path.split('/').some(p => !p || p === '.' || p === '..') || !validHex(data, MAX_ASSET) || !validHex(hash, 64) || hash.length !== 64) return fail('Invalid sample or sample exceeds 6 MiB');
        if (Object.hasOwn(s.assets, path)) {
          if (s.assets[path].hash === hash) return tell(ws, {type: 'asset_ack', path});
          return fail('Sample name already in room; rename the file before retrying');
        }
        if (s.bytes + data.length > ROOM_ASSETS || Object.keys(s.assets).length >= 128) return fail('Room sample limit reached');
        const key = String(Object.keys(s.assets).length);
        const next = {...s, bytes: s.bytes + data.length, assets: {...s.assets, [path]: {key, hash}}};
        await this.put(`asset/${key}`, data, next);
        this.state = next;
        tell(ws, {type: 'asset_ack', path});
        this.others(ws, {type: 'asset_available', path});
        break;
      }
      case 'leave': {
        if (s.solo) await this.finish();
        else if (a.role === 'host') await this.promote(now);
        else { s.guest = null; s.lock = null; await this.persist(); ws.close(1000, 'Left room'); this.others(ws, {type: 'presence', name: null}); }
        break;
      }
    }
  }
  async webSocketClose(ws) {
    await this.ctx.blockConcurrencyWhile(() => this.disconnect(ws));
  }
  async disconnect(ws) {
    const s = this.state;
    const a = ws.deserializeAttachment();
    if (!s || s.solo || s[a.role]?.token !== a.token) return;
    if (s.lock?.role === a.role) s.lock = null;
    s[a.role].until = Date.now() + (a.role === 'guest' ? GRACE : HOST_TIMEOUT);
    await this.persist();
    this.others(ws, {type: 'presence', name: null});
    this.others(ws, {type: 'drag', clip: null});
  }
  async webSocketError(ws) { await this.webSocketClose(ws); }
}
