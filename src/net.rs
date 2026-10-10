//! LAN co-op: two players, two frogs, one board.
//!
//! The connection is a plain TCP socket (port [`PORT`]), so it works on a
//! real LAN and over virtual LANs such as Radmin VPN or Hamachi: the guest
//! types the host's IP (the 26.x.x.x address Radmin shows).
//!
//! Both machines run the same deterministic simulation in *lockstep*: the
//! board only depends on its seed and on the inputs of both frogs, so each
//! side sends its input for every tick (scheduled [`INPUT_DELAY`] ticks ahead
//! to hide latency) and a tick runs only once both inputs are known. The
//! host decides everything else (which level, continue / retry) and sends it
//! as control messages. A checksum is exchanged every second to detect a
//! desync.

use crate::game::board::BoardInput;
use bevy::math::Vec2;
use bevy::prelude::*;
use std::collections::BTreeMap;
use std::io::{BufRead, BufReader, Write};
use std::net::{SocketAddr, TcpListener, TcpStream, ToSocketAddrs, UdpSocket};
use std::sync::Mutex;
use std::sync::mpsc::{Receiver, TryRecvError, channel};
use std::time::Duration;

pub const PORT: u16 = 27015;
/// Protocol version; both sides must match.
const VERSION: u32 = 1;
/// Ticks between sampling an input and simulating it (80 ms).
pub const INPUT_DELAY: u32 = 8;
/// Ticks between checksum exchanges.
pub const SUM_EVERY: u32 = 100;

/// What the host tells the guest to play.
#[derive(Clone, Debug, PartialEq)]
pub struct StartInfo {
    /// `adventure` or `survival`.
    pub mode: String,
    pub stage: usize,
    pub level: usize,
    pub graphics: String,
    pub seed: u64,
}

#[derive(Clone, Debug, PartialEq)]
pub enum Msg {
    Hello {
        version: u32,
        data: u64,
    },
    Start(StartInfo),
    /// `board` numbers the boards of the session so late inputs of a
    /// finished board are not applied to the next one.
    Input {
        board: u64,
        tick: u32,
        input: BoardInput,
    },
    /// Host decision for the open dialog: `go` (continue / try again) or `menu`.
    Ctl(String),
    Sum {
        board: u64,
        tick: u32,
        sum: u64,
    },
    Bye,
}

impl Msg {
    pub fn encode(&self) -> String {
        match self {
            Msg::Hello { version, data } => format!("HELLO {version} {data}"),
            Msg::Start(s) => format!("START {} {} {} {} {}", s.mode, s.stage, s.level, s.seed, s.graphics),
            Msg::Input { board, tick, input } => {
                format!("IN {board} {tick} {:.2} {:.2} {} {}", input.aim.x, input.aim.y, input.fire as u8, input.swap as u8)
            }
            Msg::Ctl(c) => format!("CTL {c}"),
            Msg::Sum { board, tick, sum } => format!("SUM {board} {tick} {sum}"),
            Msg::Bye => "BYE".into(),
        }
    }

    pub fn decode(line: &str) -> Option<Msg> {
        let mut it = line.split_whitespace();
        let kind = it.next()?;
        let mut num = || it.next().map(str::to_string);
        Some(match kind {
            "HELLO" => Msg::Hello { version: num()?.parse().ok()?, data: num()?.parse().ok()? },
            "START" => {
                let mode = num()?;
                let stage = num()?.parse().ok()?;
                let level = num()?.parse().ok()?;
                let seed = num()?.parse().ok()?;
                let graphics = num().unwrap_or_default();
                Msg::Start(StartInfo { mode, stage, level, graphics, seed })
            }
            "IN" => {
                let board = num()?.parse().ok()?;
                let tick = num()?.parse().ok()?;
                let x: f32 = num()?.parse().ok()?;
                let y: f32 = num()?.parse().ok()?;
                let fire = num()? == "1";
                let swap = num()? == "1";
                Msg::Input { board, tick, input: BoardInput { aim: Vec2::new(x, y), fire, swap } }
            }
            "CTL" => Msg::Ctl(num()?),
            "SUM" => Msg::Sum { board: num()?.parse().ok()?, tick: num()?.parse().ok()?, sum: num()?.parse().ok()? },
            "BYE" => Msg::Bye,
            _ => return None,
        })
    }
}

/// An open connection to the other player.
pub struct Link {
    stream: TcpStream,
    /// In a mutex only so the link can live in a Bevy resource (`Sync`).
    rx: Mutex<Receiver<Option<Msg>>>,
    pub peer: SocketAddr,
    pub closed: bool,
}

impl Link {
    fn new(stream: TcpStream) -> std::io::Result<Self> {
        stream.set_nodelay(true)?;
        stream.set_nonblocking(false)?;
        let peer = stream.peer_addr()?;
        let reader = stream.try_clone()?;
        let (tx, rx) = channel();
        std::thread::spawn(move || {
            for line in BufReader::new(reader).lines() {
                let Ok(line) = line else { break };
                if let Some(m) = Msg::decode(&line) {
                    if tx.send(Some(m)).is_err() {
                        return;
                    }
                }
            }
            let _ = tx.send(None);
        });
        Ok(Self { stream, rx: Mutex::new(rx), peer, closed: false })
    }

    pub fn send(&mut self, m: &Msg) {
        if self.closed {
            return;
        }
        let mut line = m.encode();
        line.push('\n');
        if self.stream.write_all(line.as_bytes()).is_err() {
            self.closed = true;
        }
    }

    /// Next received message, if any.
    pub fn poll(&mut self) -> Option<Msg> {
        let rx = self.rx.get_mut().unwrap_or_else(|e| e.into_inner());
        match rx.try_recv() {
            Ok(Some(m)) => Some(m),
            Ok(None) | Err(TryRecvError::Disconnected) => {
                self.closed = true;
                None
            }
            Err(TryRecvError::Empty) => None,
        }
    }
}

impl Drop for Link {
    fn drop(&mut self) {
        let _ = self.stream.write_all(b"BYE\n");
        let _ = self.stream.shutdown(std::net::Shutdown::Both);
    }
}

/// Waits for a guest without blocking the game.
pub struct Listener(TcpListener);

impl Listener {
    pub fn bind() -> std::io::Result<Self> {
        let l = TcpListener::bind(("0.0.0.0", PORT))?;
        l.set_nonblocking(true)?;
        Ok(Self(l))
    }

    pub fn accept(&self) -> Option<std::io::Result<Link>> {
        match self.0.accept() {
            Ok((s, _)) => Some(Link::new(s)),
            Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => None,
            Err(e) => Some(Err(e)),
        }
    }
}

/// Connects in a background thread (a VPN can take a few seconds).
pub fn connect(addr: &str) -> Mutex<Receiver<Result<Link, String>>> {
    let (tx, rx) = channel();
    let addr = if addr.contains(':') { addr.to_string() } else { format!("{addr}:{PORT}") };
    std::thread::spawn(move || {
        let r = (|| {
            let sa = addr.to_socket_addrs().map_err(|e| e.to_string())?.next().ok_or("bad address")?;
            let s = TcpStream::connect_timeout(&sa, Duration::from_secs(8)).map_err(|e| e.to_string())?;
            Link::new(s).map_err(|e| e.to_string())
        })();
        let _ = tx.send(r);
    });
    Mutex::new(rx)
}

/// Local addresses to tell the other player: the default route and the
/// Radmin VPN (26.0.0.0/8) / Hamachi (25.0.0.0/8) adapters if present.
/// No packet is sent: connecting a UDP socket only picks the route.
pub fn local_ips() -> Vec<String> {
    let mut out = Vec::new();
    for probe in ["26.0.0.1:9", "25.0.0.1:9", "192.168.0.1:9", "8.8.8.8:9"] {
        let ip = UdpSocket::bind("0.0.0.0:0").and_then(|s| s.connect(probe).and_then(|_| s.local_addr())).map(|a| a.ip());
        if let Ok(ip) = ip {
            let s = ip.to_string();
            if !ip.is_unspecified() && !out.contains(&s) {
                out.push(s);
            }
        }
    }
    out
}

/// A fingerprint of the game data so both players use the same levels.
pub fn data_fingerprint(levels_xml: &[u8]) -> u64 {
    // FNV-1a.
    levels_xml.iter().fold(0xcbf29ce484222325u64, |h, &b| (h ^ b as u64).wrapping_mul(0x100000001b3))
}

pub fn hello(levels_xml: &[u8]) -> Msg {
    Msg::Hello { version: VERSION, data: data_fingerprint(levels_xml) }
}

pub fn check_hello(m: &Msg, levels_xml: &[u8]) -> Result<(), String> {
    match m {
        Msg::Hello { version, .. } if *version != VERSION => Err(format!("versión distinta ({version} != {VERSION})")),
        Msg::Hello { data, .. } if *data != data_fingerprint(levels_xml) => {
            Err("los niveles (levels.xml) no son iguales en los dos PCs".into())
        }
        Msg::Hello { .. } => Ok(()),
        _ => Err("respuesta inesperada".into()),
    }
}

/// The co-op session: the link plus the lockstep state of the current board.
#[derive(Resource)]
pub struct Coop {
    pub link: Link,
    pub host: bool,
    /// Simulated ticks of the current board.
    pub tick: u32,
    /// Next tick to sample a local input for.
    local_next: u32,
    inputs: BTreeMap<(u64, u32), [Option<BoardInput>; 2]>,
    pending_fire: bool,
    pending_swap: bool,
    sums: BTreeMap<(u64, u32), [Option<u64>; 2]>,
    pub desync: bool,
    /// Control messages from the host not yet handled.
    pub ctl: Vec<String>,
    pub start: Option<StartInfo>,
    /// Ticks spent waiting for the other player in a row.
    pub stalled: u32,
    pub base_seed: u64,
    pub boards: u64,
}

impl Coop {
    pub fn new(link: Link, host: bool, base_seed: u64) -> Self {
        let mut c = Self {
            link,
            host,
            tick: 0,
            local_next: 0,
            inputs: BTreeMap::new(),
            pending_fire: false,
            pending_swap: false,
            sums: BTreeMap::new(),
            desync: false,
            ctl: Vec::new(),
            start: None,
            stalled: 0,
            base_seed,
            boards: 0,
        };
        c.reset_board();
        c
    }

    /// Index of this machine's frog.
    pub fn me(&self) -> usize {
        if self.host { 0 } else { 1 }
    }

    /// Seed for the next board (both sides build boards in the same order).
    pub fn next_seed(&mut self) -> u64 {
        self.boards += 1;
        self.base_seed.wrapping_mul(6364136223846793005).wrapping_add(self.boards)
    }

    /// Starts the lockstep of a new board. The first ticks have no input.
    pub fn reset_board(&mut self) {
        self.tick = 0;
        self.local_next = INPUT_DELAY;
        // Keep what already arrived for this board (the other side may be ahead).
        let b = self.boards;
        self.inputs.retain(|k, _| k.0 >= b);
        self.sums.retain(|k, _| k.0 >= b);
        self.stalled = 0;
        for t in 0..INPUT_DELAY {
            self.inputs.insert((b, t), [Some(BoardInput::default()), Some(BoardInput::default())]);
        }
    }

    /// Reads the socket.
    pub fn pump(&mut self) {
        while let Some(m) = self.link.poll() {
            match m {
                Msg::Input { board, tick, input } => {
                    let other = 1 - self.me();
                    if board > self.boards || (board == self.boards && tick >= self.tick) {
                        self.inputs.entry((board, tick)).or_default()[other] = Some(input);
                    }
                }
                Msg::Sum { board, tick, sum } => {
                    let other = 1 - self.me();
                    if board >= self.boards {
                        self.sums.entry((board, tick)).or_default()[other] = Some(sum);
                    }
                }
                Msg::Ctl(c) => self.ctl.push(c),
                Msg::Start(s) => self.start = Some(s),
                Msg::Bye => self.link.closed = true,
                Msg::Hello { .. } => {}
            }
        }
        let done: Vec<(u64, u32)> = self.sums.iter().filter(|(_, v)| v[0].is_some() && v[1].is_some()).map(|(k, _)| *k).collect();
        for k in done {
            let v = self.sums.remove(&k).unwrap_or_default();
            if v[0] != v[1] {
                self.desync = true;
                warn!("co-op desync at board {} tick {}", k.0, k.1);
            } else if k.1 % 1000 == 0 {
                info!("co-op in sync: board {} tick {}", k.0, k.1);
            }
        }
    }

    /// Remembers this frame's clicks until they are sent with an input.
    pub fn local_events(&mut self, fire: bool, swap: bool) {
        self.pending_fire |= fire;
        self.pending_swap |= swap;
    }

    /// Samples the local input for every tick up to `tick + INPUT_DELAY`.
    pub fn send_inputs(&mut self, aim: Vec2) {
        while self.local_next <= self.tick + INPUT_DELAY {
            let input = BoardInput { aim, fire: std::mem::take(&mut self.pending_fire), swap: std::mem::take(&mut self.pending_swap) };
            let t = self.local_next;
            let me = self.me();
            self.inputs.entry((self.boards, t)).or_default()[me] = Some(input);
            self.link.send(&Msg::Input { board: self.boards, tick: t, input });
            self.local_next += 1;
        }
    }

    /// Inputs of both frogs for the current tick, if they arrived.
    pub fn ready(&self) -> Option<[BoardInput; 2]> {
        let v = self.inputs.get(&(self.boards, self.tick))?;
        Some([v[0]?, v[1]?])
    }

    /// Marks the current tick as simulated; `sum` is the board checksum.
    pub fn advance(&mut self, sum: impl FnOnce() -> u64) {
        self.inputs.remove(&(self.boards, self.tick));
        self.tick += 1;
        self.stalled = 0;
        if self.tick % SUM_EVERY == 0 {
            let s = sum();
            let me = self.me();
            self.sums.entry((self.boards, self.tick)).or_default()[me] = Some(s);
            self.link.send(&Msg::Sum { board: self.boards, tick: self.tick, sum: s });
        }
    }

    /// Host only: tells the guest what the dialog decision was.
    pub fn send_ctl(&mut self, c: &str) {
        self.link.send(&Msg::Ctl(c.to_string()));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn messages_roundtrip() {
        let msgs = [
            Msg::Hello { version: 1, data: 42 },
            Msg::Start(StartInfo { mode: "adventure".into(), stage: 2, level: 3, graphics: "spiral".into(), seed: 99 }),
            Msg::Input { board: 3, tick: 7, input: BoardInput { aim: Vec2::new(12.5, 300.25), fire: true, swap: false } },
            Msg::Ctl("go".into()),
            Msg::Sum { board: 1, tick: 100, sum: u64::MAX },
            Msg::Bye,
        ];
        for m in msgs {
            assert_eq!(Msg::decode(&m.encode()), Some(m));
        }
    }

    #[test]
    fn lockstep_over_loopback() {
        let l = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = l.local_addr().unwrap();
        let guest = std::thread::spawn(move || Link::new(TcpStream::connect(addr).unwrap()).unwrap());
        let host_link = Link::new(l.accept().unwrap().0).unwrap();
        let mut h = Coop::new(host_link, true, 5);
        let mut g = Coop::new(guest.join().unwrap(), false, 5);
        assert_eq!(h.next_seed(), g.next_seed());
        h.reset_board();
        g.reset_board();
        let mut steps = 0;
        while h.tick < 50 || g.tick < 50 {
            for c in [&mut h, &mut g] {
                c.pump();
                c.local_events(true, false);
                c.send_inputs(Vec2::ONE);
                while let Some(both) = c.ready() {
                    assert_eq!(both[0].aim, both[1].aim);
                    c.advance(|| 1);
                    if c.tick >= 50 {
                        break;
                    }
                }
            }
            steps += 1;
            assert!(steps < 10_000);
            std::thread::sleep(Duration::from_millis(1));
        }
        assert!(!h.desync && !g.desync);
    }
}
