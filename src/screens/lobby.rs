//! LAN co-op lobby: host a game or join one by IP (works over Radmin VPN).

use super::{Mode, Screen, Session};
use crate::assets::GameAssets;
use crate::audio::Sfx;
use crate::debug::DebugOptions;
use crate::gfx::Gfx;
use crate::input::Mouse;
use crate::net::{self, Coop, Link, Listener, Msg, StartInfo};
use crate::profile::Profile;
use crate::ui::{dialog_box, dialog_button};
use bevy::input::ButtonState;
use bevy::input::keyboard::{Key, KeyboardInput};
use bevy::prelude::*;
use std::sync::Mutex;
use std::sync::mpsc::Receiver;

#[derive(Default)]
enum Phase {
    #[default]
    Choose,
    Hosting(Option<Listener>),
    Connecting(Mutex<Receiver<Result<Link, String>>>),
    /// Connected; waiting for the other side's HELLO.
    Handshake(Link, bool),
    /// Connected and verified; `true` on the host.
    Ready(Link, bool),
}

#[derive(Default)]
pub struct LobbyState {
    phase: Phase,
    ip: String,
    error: String,
    ips: Vec<String>,
    stage: usize,
    survival: bool,
    auto_done: bool,
    frames: u32,
}

#[derive(Resource, Default)]
pub struct Lobby(LobbyState);

pub fn enter(mut lobby: ResMut<Lobby>, profile: Res<Profile>) {
    lobby.0 = LobbyState { ip: profile.last_ip.clone(), stage: profile.max_stage, ips: net::local_ips(), ..default() };
}

#[allow(clippy::too_many_arguments)]
pub fn update(
    mut commands: Commands,
    mut lobby: ResMut<Lobby>,
    assets: Res<GameAssets>,
    mut g: ResMut<Gfx>,
    mut mouse: ResMut<Mouse>,
    mut sfx: ResMut<Sfx>,
    mut keys: MessageReader<KeyboardInput>,
    mut next: ResMut<NextState<Screen>>,
    mut session: ResMut<Session>,
    mut profile: ResMut<Profile>,
    debug: Res<DebugOptions>,
) {
    let st = &mut lobby.0;
    st.frames += 1;
    let a = &*assets;
    let levels_xml = a.dir.read("levels/levels.xml").unwrap_or_default();

    // Command line: --host / --join IP.
    if !st.auto_done {
        st.auto_done = true;
        if debug.host {
            st.phase = start_hosting(&mut st.error);
        } else if let Some(ip) = &debug.join {
            st.ip = ip.clone();
            st.phase = Phase::Connecting(net::connect(ip));
        }
    }

    // IP typing.
    for k in keys.read() {
        if k.state != ButtonState::Pressed || !matches!(st.phase, Phase::Choose) {
            continue;
        }
        match &k.logical_key {
            Key::Character(c) => {
                for ch in c.chars().filter(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '.' | ':' | '-')) {
                    if st.ip.len() < 40 {
                        st.ip.push(ch);
                    }
                }
            }
            Key::Backspace => {
                st.ip.pop();
            }
            _ => {}
        }
    }

    // Network progress.
    let phase = std::mem::take(&mut st.phase);
    st.phase = match phase {
        Phase::Hosting(Some(l)) => match l.accept() {
            Some(Ok(mut link)) => {
                link.send(&net::hello(&levels_xml));
                Phase::Handshake(link, true)
            }
            Some(Err(e)) => {
                st.error = e.to_string();
                Phase::Hosting(Some(l))
            }
            None => Phase::Hosting(Some(l)),
        },
        Phase::Connecting(mut rx) => match rx.get_mut().unwrap_or_else(|e| e.into_inner()).try_recv() {
            Ok(Ok(mut link)) => {
                link.send(&net::hello(&levels_xml));
                Phase::Handshake(link, false)
            }
            Ok(Err(e)) => {
                st.error = format!("NO SE PUDO CONECTAR: {e}");
                Phase::Choose
            }
            Err(_) => Phase::Connecting(rx),
        },
        Phase::Handshake(mut link, host) => match link.poll() {
            Some(m) => match net::check_hello(&m, &levels_xml) {
                Ok(()) => Phase::Ready(link, host),
                Err(e) => {
                    st.error = e;
                    Phase::Choose
                }
            },
            None if link.closed => {
                st.error = "CONEXIÓN CERRADA".into();
                Phase::Choose
            }
            None => Phase::Handshake(link, host),
        },
        other => other,
    };

    // The guest waits for START.
    if let Phase::Ready(link, false) = &mut st.phase {
        {
            match link.poll() {
                Some(Msg::Start(info)) => {
                    let Phase::Ready(link, _) = std::mem::take(&mut st.phase) else { unreachable!() };
                    begin(&mut commands, &mut session, &mut next, link, false, &info);
                    return;
                }
                Some(Msg::Bye) => st.error = "EL ANFITRIÓN SE FUE".into(),
                _ => {}
            }
            if link.closed {
                st.phase = Phase::Choose;
                st.error = "CONEXIÓN CERRADA".into();
            }
        }
    }

    // Drawing.
    g.fill_rect(0.0, 0.0, 640.0, 480.0, Color::srgb(0.08, 0.06, 0.04));
    let (x, y, w, h) = (120.0, 50.0, 400.0, 380.0);
    dialog_box(&mut g, a, x, y, w, h, "CO-OP LAN");
    let f = a.font("FONT_MAIN12");
    let small = a.font("FONT_MAIN10");
    let col = Color::srgb(1.0, 0.9, 0.6);
    let white = Color::WHITE;
    let cx = x + w * 0.5;
    if !st.error.is_empty() {
        g.text_centered(small, &st.error.to_uppercase(), cx, y + h - 22.0, Color::srgb(1.0, 0.4, 0.3));
    }

    let back;
    match &mut st.phase {
        Phase::Choose => {
            g.text_centered(small, "DOS RANAS, UN TABLERO. FUNCIONA EN LAN Y CON RADMIN VPN.", cx, y + 85.0, col);
            if dialog_button(&mut g, a, &mut mouse, &mut sfx, "CREAR PARTIDA", cx - 90.0, y + 105.0, 180.0) {
                st.error.clear();
                st.phase = start_hosting(&mut st.error);
            }
            g.text_centered(f, "IP DEL ANFITRIÓN:", cx, y + 185.0, col);
            g.fill_rect(cx - 120.0, y + 195.0, 240.0, 26.0, Color::srgba(0.0, 0.0, 0.0, 0.6));
            let caret = if (st.frames / 30) % 2 == 0 { "_" } else { " " };
            g.text_centered(f, &format!("{}{caret}", st.ip), cx, y + 214.0, white);
            g.text_centered(small, "ESCRIBE LA IP (P. EJ. 26.12.34.56) Y PULSA UNIRSE", cx, y + 238.0, col);
            if dialog_button(&mut g, a, &mut mouse, &mut sfx, "UNIRSE", cx - 90.0, y + 250.0, 180.0) && !st.ip.is_empty() {
                st.error.clear();
                profile.last_ip = st.ip.clone();
                profile.save();
                st.phase = Phase::Connecting(net::connect(&st.ip));
            }
            back = dialog_button(&mut g, a, &mut mouse, &mut sfx, "VOLVER", cx - 90.0, y + 300.0, 180.0);
        }
        Phase::Hosting(_) => {
            g.text_centered(f, "ESPERANDO AL OTRO JUGADOR...", cx, y + 95.0, col);
            g.text_centered(small, &format!("PUERTO TCP {}", net::PORT), cx, y + 120.0, white);
            g.text_centered(small, "DILE A TU AMIGO UNA DE ESTAS IP:", cx, y + 145.0, col);
            for (i, ip) in st.ips.iter().enumerate().take(4) {
                let tag = if ip.starts_with("26.") {
                    "  (RADMIN VPN)"
                } else if ip.starts_with("25.") {
                    "  (HAMACHI)"
                } else {
                    ""
                };
                g.text_centered(f, &format!("{ip}{tag}"), cx, y + 170.0 + i as f32 * 22.0, white);
            }
            back = dialog_button(&mut g, a, &mut mouse, &mut sfx, "CANCELAR", cx - 90.0, y + 300.0, 180.0);
        }
        Phase::Connecting(_) | Phase::Handshake(..) => {
            g.text_centered(f, "CONECTANDO...", cx, y + 150.0, col);
            back = dialog_button(&mut g, a, &mut mouse, &mut sfx, "CANCELAR", cx - 90.0, y + 300.0, 180.0);
        }
        Phase::Ready(link, host) => {
            let host = *host;
            g.text_centered(small, &format!("CONECTADO CON {}", link.peer.ip()), cx, y + 85.0, Color::srgb(0.6, 1.0, 0.6));
            if host {
                let stages = a.levels.stages.len().max(1);
                let max = profile.max_stage.min(stages - 1);
                st.stage = st.stage.min(max);
                let mode = if st.survival { "MODO: GAUNTLET SURVIVAL" } else { "MODO: AVENTURA" };
                if dialog_button(&mut g, a, &mut mouse, &mut sfx, mode, cx - 130.0, y + 100.0, 260.0) {
                    st.survival = !st.survival;
                }
                if !st.survival {
                    g.text_centered(f, &format!("ETAPA {}", st.stage + 1), cx, y + 175.0, white);
                    if dialog_button(&mut g, a, &mut mouse, &mut sfx, "<", cx - 130.0, y + 155.0, 60.0) {
                        st.stage = st.stage.saturating_sub(1);
                    }
                    if dialog_button(&mut g, a, &mut mouse, &mut sfx, ">", cx + 70.0, y + 155.0, 60.0) {
                        st.stage = (st.stage + 1).min(max);
                    }
                }
                let auto = debug.host && st.frames > 30;
                if dialog_button(&mut g, a, &mut mouse, &mut sfx, "EMPEZAR", cx - 90.0, y + 250.0, 180.0) || auto {
                    let stage = debug.level.map(|l| l.0).unwrap_or(st.stage);
                    let level = debug.level.map(|l| l.1).unwrap_or(0);
                    let graphics = a.levels.gauntlet.first().map(|g| g.graphics.clone()).unwrap_or_default();
                    let mode = if st.survival { "survival" } else { "adventure" };
                    let info = StartInfo { mode: mode.into(), stage, level, graphics, seed: rand::random() };
                    let Phase::Ready(mut link, _) = std::mem::take(&mut st.phase) else { unreachable!() };
                    link.send(&Msg::Start(info.clone()));
                    begin(&mut commands, &mut session, &mut next, link, true, &info);
                    return;
                }
            } else {
                g.text_centered(f, "ESPERANDO A QUE EL ANFITRIÓN", cx, y + 150.0, col);
                g.text_centered(f, "ELIJA EL NIVEL...", cx, y + 175.0, col);
            }
            back = dialog_button(&mut g, a, &mut mouse, &mut sfx, "SALIR", cx - 90.0, y + 300.0, 180.0);
        }
    }
    if back {
        if matches!(st.phase, Phase::Choose) {
            next.set(Screen::MainMenu);
        }
        // Dropping the listener / link closes the connection.
        st.phase = Phase::Choose;
        st.error.clear();
    }
}

fn start_hosting(error: &mut String) -> Phase {
    match Listener::bind() {
        Ok(l) => Phase::Hosting(Some(l)),
        Err(e) => {
            *error = format!("NO SE PUDO ABRIR EL PUERTO {}: {e}", net::PORT);
            Phase::Choose
        }
    }
}

fn begin(commands: &mut Commands, session: &mut Session, next: &mut NextState<Screen>, link: Link, host: bool, info: &StartInfo) {
    *session = if info.mode == "survival" {
        Session { mode: Mode::Survival, graphics: info.graphics.clone(), ..default() }
    } else {
        Session { stage: info.stage, level: info.level, ..Session::adventure(info.stage) }
    };
    commands.insert_resource(Coop::new(link, host, info.seed));
    next.set(Screen::Playing);
}
