//! Android transport for the existing live document model. All song mutations stay on the app thread.

use super::{HoneydewApp, live};
use crate::{collaboration::checkpoint, project_files::read_file_limited};
use eframe::egui;
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::{collections::BTreeSet, time::Duration};

const PROJECT_LIMIT: usize = 1 << 20;
const ASSET_LIMIT: usize = 6 << 20;
type Clip = (i32, usize, u32);

#[derive(Default)]
pub(super) struct Rooms {
    show: bool,
    endpoint: String,
    name: String,
    input: String,
    info: String,
    session: Option<Session>,
    pub(super) play: Option<bool>,
    follow: bool,
    monitor: bool,
    pub(super) monitored: Vec<i32>,
}

struct Session {
    host: bool,
    code: String,
    token: String,
    online: bool,
    retry: f64,
    deadline: f64,
    revision: u64,
    base: Option<Vec<u8>>,
    incoming: Option<(u64, Vec<u8>)>,
    sent: Option<Vec<u8>>,
    sent_at: f64,
    last_play: bool,
    last_solo: Vec<i32>,
    assets: BTreeSet<String>,
    wanted: BTreeSet<String>,
    fetching: Option<(String, f64)>,
    pending_asset: Option<String>,
    peer: String,
    drag: Option<Clip>,
    drag_at: f64,
    locked: Option<(Clip, f64)>,
    conflict: bool,
    ending: bool,
}

impl Session {
    fn new(host: bool, code: String, now: f64, playing: bool) -> Self {
        Self {
            host,
            code,
            token: String::new(),
            online: false,
            retry: now + 3.0,
            deadline: now + 15.0,
            revision: 0,
            base: None,
            incoming: None,
            sent: None,
            sent_at: 0.0,
            last_play: playing,
            last_solo: Vec::new(),
            assets: BTreeSet::new(),
            wanted: BTreeSet::new(),
            fetching: None,
            pending_asset: None,
            peer: String::new(),
            drag: None,
            drag_at: 0.0,
            locked: None,
            conflict: false,
            ending: false,
        }
    }
}

fn encode(data: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut text = String::with_capacity(data.len() * 2);
    for &b in data {
        text.push(HEX[(b >> 4) as usize] as char);
        text.push(HEX[(b & 15) as usize] as char);
    }
    text
}

fn decode(data: &str, max: usize) -> Option<Vec<u8>> {
    if data.is_empty() || data.len() > max * 2 || !data.len().is_multiple_of(2) {
        return None;
    }
    let hex = |b: u8| match b {
        b'0'..=b'9' => Some(b - b'0'),
        b'a'..=b'f' => Some(b - b'a' + 10),
        _ => None,
    };
    data.as_bytes()
        .chunks_exact(2)
        .map(|p| Some(hex(p[0])? * 16 + hex(p[1])?))
        .collect()
}

fn send(message: Value) -> bool {
    crate::android::room_send(&message.to_string())
}

fn locked_id() -> egui::Id {
    egui::Id::new("honeydew-room-drag")
}

pub(crate) fn locked_clip(ctx: &egui::Context) -> Option<Clip> {
    ctx.data(|d| d.get_temp::<(Clip, f64)>(locked_id()))
        .filter(|&(_, until)| ctx.input(|i| i.time) < until)
        .map(|(clip, _)| clip)
}

impl HoneydewApp {
    pub(super) fn room_frame(&mut self, ctx: &egui::Context, tick: u64) {
        // Taking the adapter out keeps UI/IO state separate from the existing app mutation methods.
        let mut rooms = std::mem::take(&mut self.rooms);
        let now = ctx.input(|i| i.time);
        let mut left = false;
        let mut rebuild = false;
        if let Some(mut s) = rooms.session.take() {
            for _ in 0..16 {
                let Some(wire) = crate::android::room_poll() else {
                    break;
                };
                let Ok(m) = serde_json::from_str::<Value>(&wire) else {
                    continue;
                };
                match m["type"].as_str().unwrap_or_default() {
                    "welcome" => {
                        s.online = true;
                        s.code = m["code"].as_str().unwrap_or_default().to_owned();
                        s.token = m["token"].as_str().unwrap_or_default().to_owned();
                        s.deadline = f64::INFINITY;
                        rooms.info = if s.host { "Hosting" } else { "Joining room…" }.into();
                    }
                    "closed" => {
                        s.online = false;
                        s.sent = None;
                        s.drag = None;
                        s.locked = None;
                        s.pending_asset = None;
                        s.fetching = None;
                        s.peer.clear();
                        rebuild |= !rooms.monitored.is_empty();
                        rooms.monitored.clear();
                        let status = m["status"].as_u64().unwrap_or(0);
                        if (!s.ending && s.token.is_empty()) || matches!(status, 400 | 404 | 410) {
                            rooms.info = match status {
                                404 | 410 => "Room not found or ended".into(),
                                409 => "Both seats are taken".into(),
                                425 => "Host project is not ready yet".into(),
                                _ => "Could not connect to the relay".into(),
                            };
                            left = true;
                        } else if !s.ending {
                            if s.deadline.is_infinite() {
                                s.deadline = now + if s.host { 15.0 } else { 60.0 };
                            }
                            s.retry = now + 1.0;
                            rooms.info = "Reconnecting — showing the last stable project".into();
                        }
                    }
                    "state" => {
                        if let Some(rev) = m["rev"].as_u64()
                            && rev >= s.revision
                            && s.incoming.as_ref().is_none_or(|(old, _)| rev >= *old)
                            && let Some(bytes) = m["checkpoint"]
                                .as_str()
                                .and_then(|v| decode(v, PROJECT_LIMIT))
                        {
                            s.incoming = Some((rev, bytes));
                            s.sent = None;
                        }
                    }
                    "ack" => {
                        if m["rev"].as_u64() == Some(s.revision + 1)
                            && let Some(bytes) = s.sent.take()
                        {
                            s.revision += 1;
                            s.base = Some(bytes);
                        }
                    }
                    "presence" => {
                        s.peer = m["name"].as_str().unwrap_or_default().to_owned();
                        s.locked = None;
                        rebuild |= !rooms.monitored.is_empty();
                        rooms.monitored.clear();
                        s.last_solo = vec![i32::MIN];
                    }
                    "drag" => {
                        s.locked = serde_json::from_value::<Clip>(m["clip"].clone())
                            .ok()
                            .map(|c| (c, now + 4.0));
                    }
                    "play" if rooms.follow => rooms.play = m["playing"].as_bool(),
                    "solo_mode" if rooms.monitor => {
                        rooms.monitored =
                            serde_json::from_value(m["channels"].clone()).unwrap_or_default();
                        rebuild = true;
                    }
                    "catalog" => {
                        if let Some(paths) = m["paths"].as_array() {
                            s.wanted.extend(
                                paths
                                    .iter()
                                    .take(128)
                                    .filter_map(Value::as_str)
                                    .filter(|p| !s.assets.contains(*p))
                                    .map(str::to_owned),
                            );
                        }
                    }
                    "asset_available" => {
                        if let Some(path) = m["path"].as_str().filter(|p| !s.assets.contains(*p)) {
                            s.wanted.insert(path.into());
                        }
                    }
                    "asset" => {
                        if let (Some(path), Some(data), Some(hash)) =
                            (m["path"].as_str(), m["data"].as_str(), m["hash"].as_str())
                        {
                            if let Some(bytes) = decode(data, ASSET_LIMIT)
                                && encode(&Sha256::digest(&bytes)) == hash
                                && self.room_asset(path, bytes)
                            {
                                s.assets.insert(path.to_owned());
                                s.wanted.remove(path);
                                s.fetching = None;
                                rebuild = true;
                            } else {
                                rooms.info = format!("Could not sync sample: {path}");
                                s.conflict = true;
                            }
                        }
                    }
                    "asset_ack" => {
                        if let Some(path) = m["path"].as_str() {
                            s.assets.insert(path.to_owned());
                            s.pending_asset = None;
                        }
                    }
                    "busy" => {
                        s.sent = None;
                        s.sent_at = now;
                    }
                    "solo" | "ended" => {
                        s.ending = true;
                        s.locked = None;
                        rooms.info = if s.host {
                            "Room ended"
                        } else {
                            "Host left. You can continue in this solo copy."
                        }
                        .into();
                        rooms.show = true;
                    }
                    "error" => {
                        rooms.info = m["message"]
                            .as_str()
                            .unwrap_or("Relay rejected an update")
                            .into();
                        s.sent = None;
                        s.pending_asset = None;
                        s.conflict = true;
                        rooms.show = true;
                    }
                    _ => {}
                }
            }
            if s.online
                && !s.conflict
                && let Some(path) = s.wanted.first()
                && s.fetching
                    .as_ref()
                    .is_none_or(|(old, at)| old != path || now >= *at + 5.0)
                && send(json!({"type": "get_asset", "path": path}))
            {
                s.fetching = Some((path.clone(), now));
            }
            if !left && self.live_quiet(ctx) && (s.wanted.is_empty() || (s.ending && s.conflict)) {
                if let Some((rev, bytes)) = s.incoming.take() {
                    let first = s.base.is_none() && !s.host;
                    let result = if first {
                        checkpoint::decode(&bytes).map(|song| {
                            self.files.room_detach();
                            self.song = song;
                            self.song_replaced(ctx);
                            self.files.song_changed();
                        })
                    } else if s.base.as_ref() == Some(&bytes) {
                        Ok(())
                    } else {
                        self.live_apply(tick, s.base.as_deref(), &bytes).map(drop)
                    };
                    match result {
                        Ok(()) => {
                            s.revision = rev;
                            s.base = Some(bytes);
                            if first {
                                rooms.info = "Guest in room".into();
                            }
                        }
                        Err(err) => {
                            s.conflict = true;
                            rooms.show = true;
                            rooms.info = format!(
                                "Edits conflict: {}. Your local project was kept. Leave to keep working independently.",
                                err.0
                            );
                        }
                    }
                }
                if s.ending {
                    send(json!({"type":"leave"}));
                    left = true;
                }
                if !left && s.online && !s.conflict && (s.host || s.base.is_some()) {
                    self.room_commit(&mut rooms, &mut s, now);
                }
            }
            if s.online && !s.ending {
                let drag = self.live_presence().piano_drag();
                if (drag != s.drag || (drag.is_some() && now >= s.drag_at + 1.0))
                    && send(json!({"type":"drag", "clip":drag}))
                {
                    s.drag = drag;
                    s.drag_at = now;
                }
                let playing = self.main_screen.playing;
                if playing != s.last_play
                    && rooms.play.is_none()
                    && send(json!({"type":"play", "playing":playing}))
                {
                    s.last_play = playing;
                }
                if let Some(playing) = rooms.play {
                    s.last_play = playing;
                }
                let solo: Vec<i32> = self
                    .song
                    .channels
                    .iter()
                    .filter(|ch| ch.params.0[3] > 0.5)
                    .map(|ch| ch.id)
                    .collect();
                if solo != s.last_solo && send(json!({"type":"solo", "channels":solo})) {
                    s.last_solo = solo;
                }
            } else if !left && !s.ending && now >= s.retry {
                if now >= s.deadline {
                    rooms.info = "Connection lost. Continuing with the local project.".into();
                    rooms.show = true;
                    left = true;
                } else if !s.token.is_empty() {
                    crate::android::room_open(&rooms.endpoint, &s.code, &rooms.name, &s.token);
                    s.retry = now + 3.0;
                }
            }
            if !left {
                rooms.session = Some(s);
            }
        }
        if left {
            crate::android::room_close();
            live::disconnect();
            rebuild |= !rooms.monitored.is_empty();
            rooms.monitored.clear();
            rooms.play = None;
        }
        let lock = rooms
            .session
            .as_ref()
            .and_then(|s| s.locked)
            .filter(|&(_, until)| now < until);
        ctx.data_mut(|d| {
            if let Some(lock) = lock {
                d.insert_temp(locked_id(), lock);
            } else {
                d.remove::<(Clip, f64)>(locked_id());
            }
        });
        self.files
            .room_guest(rooms.session.as_ref().is_some_and(|s| !s.host));
        if rooms.ui(
            ctx,
            self.files.modified(),
            self.main_screen.playing,
            lock.is_some(),
            now,
        ) {
            rebuild = true;
        }
        if rooms.session.is_some() {
            ctx.request_repaint_after(Duration::from_millis(100));
        }
        self.rooms = rooms;
        if rebuild {
            self.rebuild_as(tick, super::Handover::Undo);
        }
    }

    fn room_commit(&self, rooms: &mut Rooms, s: &mut Session, now: f64) {
        if now < s.sent_at + 0.3 || s.pending_asset.is_some() {
            return;
        }
        if s.sent.is_some() {
            if now >= s.sent_at + 3.0 && send(json!({"type":"sync"})) {
                s.sent_at = now;
            }
            return;
        }
        if s.locked.is_some_and(|(_, until)| now < until) {
            return;
        }
        for path in live::user_media(&self.song) {
            if s.assets.contains(&path) {
                continue;
            }
            let asset = self
                .files
                .resolve(&room_path(&path))
                .and_then(|p| std::fs::File::open(p).ok())
                .and_then(|f| read_file_limited(f, ASSET_LIMIT as u64).ok());
            let Some(data) = asset.filter(|v| !v.is_empty()) else {
                rooms.info = format!("Missing sample or exceeds 6 MiB: {path}");
                rooms.show = true;
                return;
            };
            if send(
                json!({"type":"asset", "path":path, "hash":encode(&Sha256::digest(&data)), "data":encode(&data)}),
            ) {
                s.pending_asset = Some(path);
            }
            return;
        }
        let Ok(checkpoint) = self.live_checkpoint() else {
            return;
        };
        if s.base.as_ref() != Some(&checkpoint) {
            if checkpoint.len() > PROJECT_LIMIT {
                rooms.info = "Project is larger than the 1 MiB room checkpoint limit".into();
                rooms.show = true;
                return;
            }
            if send(json!({"type":"commit", "base":s.revision, "checkpoint":encode(&checkpoint)})) {
                s.sent = Some(checkpoint);
                s.sent_at = now;
            }
        } else {
            s.sent_at = now; // throttle idle checkpoint encoding too
        }
    }

    fn room_asset(&mut self, relative: &str, data: Vec<u8>) -> bool {
        const FOLDERS: &[&str] = &[
            "My Samples",
            "My Recordings",
            "My Instruments",
            "My SoundFonts",
        ];
        const EXTENSIONS: &[&str] = &[
            "wav", "flac", "ogg", "mp3", "aif", "aiff", "sf2", "sfz", "dwp",
        ];
        let allowed = relative
            .split_once('/')
            .is_some_and(|(dir, _)| FOLDERS.contains(&dir))
            && relative
                .rsplit_once('.')
                .is_some_and(|(_, ext)| EXTENSIONS.contains(&ext.to_ascii_lowercase().as_str()));
        if !allowed
            || relative.contains(['\\', ':', '\0'])
            || relative.split('/').any(|p| matches!(p, "" | "." | ".."))
        {
            return false;
        }
        let key = room_path(relative);
        let Some(path) = self.files.resolve(&key) else {
            return false;
        };
        if path.exists() {
            return std::fs::File::open(path)
                .ok()
                .and_then(|f| read_file_limited(f, ASSET_LIMIT as u64).ok())
                .is_some_and(|old| old == data);
        }
        let Some(root) = self.files.user_dir().and_then(|p| p.canonicalize().ok()) else {
            return false;
        };
        let Some(parent) = path.parent() else {
            return false;
        };
        if std::fs::create_dir_all(parent).is_err()
            || !parent.canonicalize().is_ok_and(|p| p.starts_with(&root))
        {
            return false;
        }
        use std::io::Write as _;
        let Ok(mut file) = std::fs::File::create_new(&path) else {
            return false;
        };
        let ok = file.write_all(&data).and_then(|()| file.sync_all()).is_ok();
        if !ok {
            let _ = std::fs::remove_file(&path);
        }
        if ok {
            self.audio
                .samples
                .forget(&crate::sample_cache::SampleKey::File(key));
        }
        ok
    }
}

fn room_path(relative: &str) -> crate::flm::chunk::ContentPath {
    use crate::flm::chunk::{ContentPath, ContentRoot};
    ContentPath::Shorthand {
        root: ContentRoot::User,
        relative: relative.into(),
    }
}

impl Rooms {
    pub(super) fn project_replaced(&mut self) {
        if self.session.take().is_some() {
            send(json!({"type":"leave"}));
            crate::android::room_close();
            live::disconnect();
            self.monitored.clear();
            self.play = None;
            self.info = "Room ended when you switched projects.".into();
        }
    }

    fn ui(
        &mut self,
        ctx: &egui::Context,
        modified: bool,
        playing: bool,
        locked: bool,
        now: f64,
    ) -> bool {
        if !self.show {
            egui::Area::new(egui::Id::new("rooms-button"))
                .anchor(egui::Align2::RIGHT_BOTTOM, [-12.0, -44.0])
                .show(ctx, |ui| {
                    if ui
                        .button(if locked {
                            "Peer moving notes"
                        } else {
                            "Collab"
                        })
                        .clicked()
                    {
                        self.show = true;
                    }
                });
            return false;
        }
        let mut visible = true;
        let mut rebuild = false;
        egui::Window::new("Collab")
            .open(&mut visible)
            .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
            .resizable(false)
            .show(ctx, |ui| {
                ui.set_max_width(305.0);
                ui.label(&self.info);
                if let Some(s) = &self.session {
                    ui.label(format!(
                        "Room {}",
                        if s.code.is_empty() { "…" } else { &s.code }
                    ));
                    if !s.peer.is_empty() {
                        ui.label(format!("With {}", s.peer));
                    }
                    if locked {
                        ui.label("Your collaborator is moving notes.");
                    }
                    if !s.code.is_empty() && ui.button("Copy invite link").clicked() {
                        ctx.copy_text(format!(
                            "{}/join?code={}",
                            self.endpoint.trim_end_matches('/'),
                            s.code
                        ));
                    }
                    ui.checkbox(&mut self.follow, "Follow collaborator playback");
                    if ui
                        .checkbox(&mut self.monitor, "Monitor collaborator channel solos")
                        .changed()
                        && !self.monitor
                    {
                        self.monitored.clear();
                        rebuild = true;
                    }
                    if ui
                        .button(if s.host { "End room" } else { "Leave room" })
                        .clicked()
                    {
                        send(json!({"type":"leave"}));
                        crate::android::room_close();
                        live::disconnect();
                        self.session = None;
                        self.play = None;
                        self.monitored.clear();
                        self.info = "Room closed. The local project is still open.".into();
                        rebuild = true;
                    }
                } else {
                    field(ui, "HTTPS relay URL", &mut self.endpoint, 240);
                    field(ui, "Display name", &mut self.name, 32);
                    field(ui, "Room code", &mut self.input, 10);
                    let create = ui.button("Create room from this song").clicked();
                    let join = ui.button("Join room").clicked();
                    if create || join {
                        let code = self.input.trim().to_ascii_uppercase();
                        if self.name.trim().is_empty() {
                            self.info = "Enter a display name".into();
                        } else if join && modified {
                            self.info =
                                "Save the current song before joining to protect your edits."
                                    .into();
                        } else if join
                            && (code.len() != 10 || !code.bytes().all(|b| b.is_ascii_hexdigit()))
                        {
                            self.info = "Enter the 10-character room code".into();
                        } else if crate::android::room_open(
                            self.endpoint.trim(),
                            if create { "new" } else { &code },
                            self.name.trim(),
                            "",
                        ) {
                            self.endpoint = self.endpoint.trim().trim_end_matches('/').into();
                            self.session = Some(Session::new(
                                create,
                                if create { String::new() } else { code },
                                now,
                                playing,
                            ));
                            self.info = "Connecting…".into();
                            live::connect();
                        } else {
                            self.info = "Enter a valid HTTPS relay URL (no path)".into();
                        }
                    }
                }
            });
        self.show = visible;
        rebuild
    }
}

fn field(ui: &mut egui::Ui, label: &str, value: &mut String, limit: usize) {
    ui.label(label);
    let mut response = ui.add(egui::TextEdit::singleline(value).char_limit(limit));
    crate::android_text::edit(&mut response, value, limit);
}
