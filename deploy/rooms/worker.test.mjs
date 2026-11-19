import {test} from 'node:test';
import assert from 'node:assert/strict';
import {Room} from './worker.mjs';

function rig() {
  const data = new Map();
  const storage = {
    get: async key => data.get(key),
    put: async (key, value) => data.set(key, structuredClone(value)),
    list: async ({prefix}) => new Map([...data].filter(([k]) => k.startsWith(prefix)).sort()),
    delete: async keys => keys.forEach(key => data.delete(key)),
    transaction: async work => work(storage),
    deleteAll: async () => data.clear(), setAlarm: async () => {}, deleteAlarm: async () => {},
  };
  const sockets = [];
  const ctx = {storage, blockConcurrencyWhile: work => work(), getWebSockets: role => sockets.filter(ws => ws.readyState === 1 && (!role || ws.deserializeAttachment().role === role))};
  const room = new Room(ctx);
  const socket = role => {
    let a = {role, token: role, start: 0, count: 0};
    const ws = {readyState: 1, received: [], deserializeAttachment: () => a,
      serializeAttachment: v => { a = v; }, send: text => ws.received.push(JSON.parse(text)), close: () => { ws.readyState = 3; }};
    sockets.push(ws);
    return ws;
  };
  return {room, socket, data};
}

async function session() {
  const r = rig();
  await Promise.resolve();
  r.room.state = {rev: 0, end: Date.now() + 120_000, host: {token: 'host', name: 'H'}, guest: {token: 'guest', name: 'G'}, assets: {}, bytes: 0};
  const host = r.socket('host');
  const guest = r.socket('guest');
  const send = (ws, msg) => r.room.webSocketMessage(ws, JSON.stringify(msg));
  return {...r, host, guest, send};
}

test('a revision is committed once; stale writers receive the authoritative checkpoint', async () => {
  const {room, host, guest, send} = await session();
  await send(host, {type: 'commit', base: 0, checkpoint: 'abcd'});
  assert.deepEqual(host.received.at(-1), {type: 'ack', rev: 1});
  assert.equal(guest.received.at(-1).checkpoint, 'abcd');
  await send(guest, {type: 'commit', base: 0, checkpoint: 'ffff'});
  assert.equal(room.state.rev, 1);
  assert.deepEqual(guest.received.at(-1), {type: 'state', rev: 1, checkpoint: 'abcd'});
});

test('the drag lease keeps the peer at the previous checkpoint until release', async () => {
  const {host, guest, send} = await session();
  const clip = [2, 0, 1];
  await send(host, {type: 'drag', clip});
  assert.deepEqual(guest.received.at(-1), {type: 'drag', clip, name: 'H'});
  await send(guest, {type: 'commit', base: 0, checkpoint: '1234'});
  assert.equal(guest.received.at(-1).type, 'busy');
  await send(host, {type: 'commit', base: 0, checkpoint: '0001'});
  await send(host, {type: 'drag', clip: null});
  assert.equal(guest.received.at(-1).clip, null);
});

test('guest seat is reserved then expires; host timeout hands the guest a solo state', async () => {
  const {room, host, guest} = await session();
  guest.close();
  await room.webSocketClose(guest);
  const expires = room.state.guest.until;
  await room.expire(expires - 1);
  assert.equal(room.state.guest.token, 'guest');
  await room.expire(expires);
  assert.equal(room.state.guest, null);
  const other = await session();
  other.host.close();
  await other.room.webSocketClose(other.host);
  await other.room.expire(other.room.state.host.until);
  assert.equal(other.guest.received.at(-1).type, 'solo');
  assert.equal(other.room.state.solo, true);
  await other.send(other.guest, {type: 'leave'});
  assert.equal(other.room.state, null);
  assert.equal(host.readyState, 1);
});

test('samples persist separately and a retry of identical content is idempotent', async () => {
  const {room, host, guest, send} = await session();
  const path = 'My Samples/Kick.wav';
  const hash = 'ab'.repeat(32);
  const asset = {type:'asset', path, hash, data:'00ff'};
  await send(host, asset);
  await send(host, asset);
  assert.equal(host.received.at(-1).type, 'asset_ack');
  assert.equal(room.state.bytes, 4);
  await send(guest, {type:'get_asset', path});
  assert.deepEqual(guest.received.at(-1), asset);
});
