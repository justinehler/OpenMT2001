//! One peer of a connected game for `game headless --host/--join`.
//!
//! The lobby is the real worker and TCP transport. Each human readies itself
//! and the host starts once the expected humans are ready, after adding any
//! computer rows. The start is the windowed game's connected start, and every
//! frame is the same `Host::frame`, on virtual frame time. The host runs
//! ahead freely; a client waits at its tick ceiling for the host's grants
//! (0x481680), which is the only synchronization between them.
use super::{End, Options, Start, line, respond};
use openmt2001::network::{
    RowKind,
    lobby::{
        Action, Settings, Snapshot,
        worker::{Handle, Open, Ticket, Update},
    },
    synchronization::Comparison,
};
use std::io::Write;
use std::net::SocketAddr;
use std::time::{Duration, Instant};

/// How long the lobby waits for the other humans or the host.
const LOBBY_TIMEOUT: Duration = Duration::from_secs(300);
/// A city whose clock stands this long (wall time) is waiting for a peer
/// that is not coming: host construction and the startup barrier fit well
/// inside it.
const STALL: Duration = Duration::from_secs(120);

#[cfg(test)]
thread_local! {
    /// In-process soak tests run each peer on a transport owner of its own.
    static ISOLATED_OWNER: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}
fn open_worker(open: Open) -> Result<Handle, String> {
    #[cfg(test)]
    if ISOLATED_OWNER.get() {
        return Handle::open_isolated(open);
    }
    Handle::open(open)
}

/// This process's place in the started game.
#[derive(Clone, Debug, Default)]
pub struct Peer {
    pub host: bool,
    /// Compact player slot (Sample sender, owner order).
    pub slot: u8,
    pub seed: u32,
    /// Compact slots of the other humans. The host waits for each one's
    /// final sample before it stops.
    pub remote: Vec<u8>,
}
impl Peer {
    fn from_ticket(ticket: &Ticket) -> Result<Self, String> {
        let rows: Vec<_> = ticket
            .view
            .snapshot
            .rows
            .iter()
            .filter(|r| matches!(r.player.kind, RowKind::Human | RowKind::Ai))
            .collect();
        let slot = rows
            .iter()
            .position(|r| r.peer == Some(ticket.view.local_peer))
            .ok_or("local peer absent from the started roster")?;
        Ok(Self {
            host: ticket.view.host,
            slot: slot as u8,
            seed: ticket.seed,
            remote: rows
                .iter()
                .enumerate()
                .filter(|(i, r)| r.peer.is_some() && *i != slot)
                .map(|(i, _)| i as u8)
                .collect(),
        })
    }
}

/// Host settings for a multiplayer level, as the host setup panel hands
/// them to the lobby (`skirmish_setup::Handoff`).
fn host_settings(
    image: &openmt2001::pe::PeImage<'_>,
    scripts: &std::path::Path,
    level: &str,
    map: &str,
    target: Option<u32>,
    difficulty: u8,
) -> Result<Settings, String> {
    let catalog = crate::campaign_store::load_catalog(image, scripts)?;
    let id = super::number(level).ok();
    let level = catalog
        .levels
        .iter()
        .filter(|l| !l.single_player() && (300000..=300006).contains(&l.id()))
        .find(|l| Some(l.id()) == id || l.directory().eq_ignore_ascii_case(level.as_bytes()))
        .ok_or(format!("--scenario {level}: not a multiplayer level"))?;
    let directory =
        std::str::from_utf8(level.directory()).map_err(|_| "level directory is not UTF-8")?;
    let hub = crate::campaign_store::original_path(scripts, &format!("{directory}/hub.lua"))?
        .ok_or(format!("{directory}/hub.lua is missing"))?;
    let hub = std::fs::read(&hub).map_err(|e| format!("{}: {e}", hub.display()))?;
    let kind =
        openmt2001::level_script::objective_selection::evaluate_setup(image, level, &hub)?.kind;
    let options = openmt2001::skirmish::Options::from_pe(image)?;
    let rule = options
        .rules()
        .iter()
        .find(|r| r.kind == kind)
        .ok_or("level objective has no host option")?;
    let target = match target {
        Some(target) if !(rule.minimum..=rule.maximum).contains(&target) => {
            return Err(format!(
                "--target must be {} to {} for this objective",
                rule.minimum, rule.maximum
            ));
        }
        Some(target) => target,
        None => rule.initial.clamp(rule.minimum, rule.maximum),
    };
    Ok(Settings {
        name: b"Headless".to_vec(),
        level: level.id(),
        parameter_bits: (target as f32).to_bits(),
        difficulty,
        map: map.as_bytes().to_vec(),
    })
}

/// The original files a connected city is staged from (`SceneInputs`).
pub type Inputs = std::sync::Arc<crate::intro::SceneInputs>;
/// A roster every peer has validated, ready to construct.
pub type Staged = crate::intro::connected_start::Prepared;

pub fn inputs(
    paths: &super::Paths,
    process: crate::live_host::process_runtime::Handle,
) -> Result<Inputs, String> {
    crate::intro::connected_start::headless_inputs(
        &paths.installer,
        &paths.scene,
        &paths.executable,
        &paths.scripts,
        &paths.language,
        process,
    )
}

pub fn stage(
    inputs: &Inputs,
    request: crate::intro::connected_start::Request,
) -> Result<Staged, String> {
    crate::intro::connected_start::stage_headless(inputs, request)
}

pub fn connect<'a>(
    inputs: &'a Inputs,
    staged: Staged,
    dispatch: Option<crate::process_dispatch::Seed>,
) -> Result<crate::fresh_city::Session<'a>, String> {
    crate::intro::connected_start::connect_headless(inputs, staged, dispatch)
}

/// Run the lobby until the worker hands over the admitted roster.
pub fn lobby(
    start: &Start,
    image: &openmt2001::pe::PeImage<'_>,
    paths: &super::Paths,
    names: &[Vec<u8>; 10],
) -> Result<(crate::intro::connected_start::Request, Peer), String> {
    let (worker, ticket) = match start {
        Start::Host {
            port,
            level,
            map,
            target,
            difficulty,
            character,
            players,
            ai,
        } => {
            let settings = host_settings(image, &paths.scripts, level, map, *target, *difficulty)?;
            let snapshot = Snapshot::host(settings, b"Host".to_vec(), *character as i8)
                .map_err(|e| e.to_string())?;
            let worker = open_worker(Open::Host {
                address: SocketAddr::from(([0, 0, 0, 0], *port)),
                snapshot,
                token_names: names.clone(),
            })?;
            let ticket = host_lobby(&worker, *players, *ai)?;
            (worker, ticket)
        }
        Start::Join { address, character } => {
            let selection = probe(*address)?;
            let worker = open_worker(Open::Join {
                selection,
                name: b"Client".to_vec(),
                token: *character as i8,
            })?;
            let ticket = client_lobby(&worker)?;
            (worker, ticket)
        }
        Start::Scenario { .. } | Start::Save { .. } => {
            return Err("not a connected start".into());
        }
    };
    let peer = Peer::from_ticket(&ticket)?;
    eprintln!(
        "lobby: started as compact slot {} of {} rows, seed {}",
        peer.slot,
        ticket
            .view
            .snapshot
            .rows
            .iter()
            .filter(|r| matches!(r.player.kind, RowKind::Human | RowKind::Ai))
            .count(),
        peer.seed
    );
    Ok((
        crate::intro::connected_start::Request { ticket, worker },
        peer,
    ))
}

fn host_lobby(worker: &Handle, players: usize, ai: usize) -> Result<Ticket, String> {
    let deadline = Instant::now() + LOBBY_TIMEOUT;
    let (mut computers_added, mut started) = (false, false);
    let mut shown = None;
    loop {
        while let Some(update) = worker.poll() {
            match update {
                Update::Lobby(view) => {
                    let rows = &view.snapshot.rows;
                    let count = |kind| rows.iter().filter(|r| r.player.kind == kind).count();
                    let (humans, computers) = (count(RowKind::Human), count(RowKind::Ai));
                    if shown != Some((humans, computers)) {
                        eprintln!("lobby: {humans}/{players} humans, {computers}/{ai} computers");
                        shown = Some((humans, computers));
                    }
                    // Humans take the first open rows as they join, so the
                    // computer rows go after them.
                    if !computers_added && humans == players {
                        let open = rows
                            .iter()
                            .enumerate()
                            .filter(|(_, r)| r.player.kind == RowKind::Open)
                            .map(|(i, _)| i);
                        for row in open.take(ai) {
                            worker.action(Action::Kind(row))?;
                        }
                        computers_added = true;
                    }
                    if !started
                        && humans == players
                        && computers == ai
                        && view.snapshot.start_enabled()
                    {
                        worker.action(Action::Start)?;
                        started = true;
                    }
                }
                Update::Prepare(ticket) => return Ok(ticket),
                Update::Notice(notice) => eprintln!("lobby: {notice}"),
                Update::Failed(error) => return Err(error),
                Update::Probed(_) | Update::Validated => (),
            }
        }
        if Instant::now() > deadline {
            return Err("timed out waiting for the humans to join and ready".into());
        }
        std::thread::sleep(Duration::from_millis(10));
    }
}

/// Probe the host until it answers; the host may still be loading.
fn probe(address: SocketAddr) -> Result<openmt2001::network::lobby::worker::Selection, String> {
    let deadline = Instant::now() + LOBBY_TIMEOUT;
    loop {
        let probe = open_worker(Open::Probe(address))?;
        let result = loop {
            match probe.poll() {
                Some(Update::Probed(selection)) => break Ok(selection),
                Some(Update::Failed(error)) => break Err(error),
                Some(_) => (),
                None => std::thread::sleep(Duration::from_millis(10)),
            }
        };
        // The process owner takes one attempt at a time; retire the probe.
        drop(probe);
        match result {
            Ok(selection) => return Ok(selection),
            Err(error) if Instant::now() > deadline => {
                return Err(format!("probe {address}: {error}"));
            }
            Err(_) => std::thread::sleep(Duration::from_millis(250)),
        }
    }
}

fn client_lobby(worker: &Handle) -> Result<Ticket, String> {
    let deadline = Instant::now() + LOBBY_TIMEOUT;
    let mut readied = false;
    loop {
        while let Some(update) = worker.poll() {
            match update {
                Update::Lobby(view) => {
                    let row = view.snapshot.local_row(view.local_peer);
                    if !readied && row.is_some_and(|row| !view.snapshot.rows[row].player.ready) {
                        worker.action(Action::Ready)?;
                        readied = true;
                    }
                }
                Update::Prepare(ticket) => return Ok(ticket),
                Update::Notice(notice) => eprintln!("lobby: {notice}"),
                Update::Failed(error) => return Err(error),
                Update::Probed(_) | Update::Validated => (),
            }
        }
        if Instant::now() > deadline {
            return Err("timed out waiting for the host to start".into());
        }
        std::thread::sleep(Duration::from_millis(10));
    }
}

/// Eight bytes of SHA-256, as hex.
fn digest(bytes: &[u8]) -> String {
    use sha2::Digest;
    sha2::Sha256::digest(bytes)[..8]
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}

/// One hourly line: day, clock, Total tick and a digest per shared tag.
pub fn hour_line(day: u32, hour: u32, tick: u32, shared: &[(u16, Vec<u8>)]) -> String {
    let tags: Vec<String> = shared
        .iter()
        .map(|(tag, bytes)| format!("\"{tag}\":\"{}\"", digest(bytes)))
        .collect();
    format!(
        "{{\"day\":{day},\"clock\":\"{hour:02}:00\",\"tick\":{tick},\"shared\":{{{}}}}}",
        tags.join(",")
    )
}

/// One line per 0x481520 comparison the host made.
pub fn sample_line(comparison: &Comparison) -> String {
    let result = if !comparison.local {
        "\"no local sample\"".to_owned()
    } else if comparison.differences.is_empty() {
        "\"match\"".to_owned()
    } else {
        let differences: Vec<String> = comparison
            .differences
            .iter()
            .map(|d| {
                format!(
                    "{{\"field\":\"{:?}\",\"local\":{},\"remote\":{}}}",
                    d.field, d.local, d.remote
                )
            })
            .collect();
        format!("\"desync\",\"differences\":[{}]", differences.join(","))
    };
    format!(
        "{{\"sample\":{},\"player\":{},\"result\":{result}}}",
        comparison.tick, comparison.sender
    )
}

fn report_hour(
    session: &mut crate::fresh_city::Session<'_>,
    image: &openmt2001::pe::PeImage<'_>,
    shared_dir: Option<&std::path::Path>,
    out: &mut dyn Write,
) -> Result<(), String> {
    let shared = crate::intro::connected_start::shared_state(session, image)?;
    let tagged: Vec<_> = crate::intro::connected_start::SHARED_TAGS
        .into_iter()
        .zip(shared)
        .collect();
    let clock = &session.world.clock;
    if let Some(dir) = shared_dir {
        std::fs::create_dir_all(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
        for (tag, bytes) in &tagged {
            let path = dir.join(format!("{}-{tag}.bin", clock.timing[0]));
            std::fs::write(&path, bytes).map_err(|e| format!("{}: {e}", path.display()))?;
        }
    }
    line(
        out,
        &hour_line(clock.calendar[4], clock.hour(), clock.timing[0], &tagged),
    )
}

/// Host-side view of the comparisons made so far.
#[derive(Default)]
struct Samples {
    seen: u64,
    differences: u64,
    /// Senders whose sample for the final tick has been compared.
    finished: Vec<u8>,
}
impl Samples {
    fn report(
        &mut self,
        session: &crate::fresh_city::Session<'_>,
        target: u32,
        out: &mut dyn Write,
    ) -> Result<(), String> {
        let Some(connection) = &session.connected else {
            return Ok(());
        };
        let comparisons = &connection.synchronization.samples.comparisons;
        for comparison in comparisons.since(self.seen) {
            line(out, &sample_line(comparison))?;
            self.differences += comparison.differences.len() as u64;
            if comparison.tick == target && !self.finished.contains(&comparison.sender) {
                self.finished.push(comparison.sender);
            }
        }
        self.seen = comparisons.count;
        Ok(())
    }
}

/// Advance one connected peer to the tick budget, then end together: the
/// host waits until it has compared every client's sample for the final
/// tick, and each client stays connected until the host has left.
pub fn advance(
    host: &mut crate::live_host::Host,
    session: &mut crate::fresh_city::Session<'_>,
    assets: &crate::fresh_city::CommandAssets<'_, '_>,
    options: &Options,
    peer: &Peer,
    out: &mut dyn Write,
) -> Result<End, String> {
    let start = session.world.clock.timing[0];
    let target = start.saturating_add(options.ticks);
    line(
        out,
        &format!(
            "{{\"connected\":\"{}\",\"slot\":{},\"seed\":{}}}",
            if peer.host { "host" } else { "client" },
            peer.slot,
            peer.seed
        ),
    )?;
    report_hour(session, assets.image, options.shared_dir.as_deref(), out)?;
    let mut samples = Samples::default();
    let mut last_tick = start;
    let mut last_hour = session.world.clock.hour();
    let mut moved = Instant::now();
    let mut still = 0;
    let end = loop {
        if session.world.clock.timing[0] >= target {
            break finish(host, session, peer, target, &mut samples, out)?;
        }
        let frame_started = Instant::now();
        host.frame(session, assets, options.frame_ms);
        // A closed barrier asks the session worker to resynchronize its
        // process clock (0x4ebaa0); virtual frame time has nothing to rebase.
        host.synchronize_time = false;
        samples.report(session, target, out)?;
        if let Some(error) = &host.failure {
            break End::Failure(error.clone());
        }
        if let Some(end) = respond(host, session, assets, options, out)? {
            break end;
        }
        let tick = session.world.clock.timing[0];
        if tick == last_tick {
            // Waiting for a peer: the startup barrier, or a client at its
            // tick ceiling. Yield the core instead of spinning.
            still += 1;
            if moved.elapsed() > STALL {
                break End::Stalled { frames: still };
            }
            std::thread::sleep(Duration::from_micros(500));
            continue;
        }
        (still, moved) = (0, Instant::now());
        let pace = Duration::from_millis(u64::from(options.pace_ms));
        if let Some(rest) = pace.checked_sub(frame_started.elapsed()) {
            std::thread::sleep(rest);
        }
        let report = match options.every {
            Some(every) => (tick - start) / every != (last_tick - start) / every,
            None => session.world.clock.hour() != last_hour,
        };
        last_tick = tick;
        last_hour = session.world.clock.hour();
        if report {
            report_hour(session, assets.image, options.shared_dir.as_deref(), out)?;
        }
    };
    let end = match end {
        End::Ticks if samples.differences > 0 => End::Desync {
            count: samples.differences,
        },
        end => end,
    };
    super::end_line(out, &end, session.world.clock.timing[0] - start)?;
    Ok(end)
}

/// Keep the link serviced without advancing the city until both sides are
/// done. Only network maintenance runs: no frame, so no logic tick.
fn finish(
    host: &mut crate::live_host::Host,
    session: &mut crate::fresh_city::Session<'_>,
    peer: &Peer,
    target: u32,
    samples: &mut Samples,
    out: &mut dyn Write,
) -> Result<End, String> {
    let deadline = Instant::now() + STALL;
    loop {
        let pumped = session.pump_connected_synchronization(&mut host.policy);
        samples.report(session, target, out)?;
        match pumped {
            // The host closing after this client's end is the normal end.
            Err(_) if !peer.host => return Ok(End::Ticks),
            Err(error) => {
                return Ok(End::Failure(format!(
                    "a peer left before its final sample: {error}"
                )));
            }
            Ok(_) => (),
        }
        if peer.host && peer.remote.iter().all(|s| samples.finished.contains(s)) {
            return Ok(End::Ticks);
        }
        if Instant::now() > deadline {
            return Ok(End::Failure(if peer.host {
                "a client never sent its final sample".into()
            } else {
                "the host never closed the game".into()
            }));
        }
        std::thread::sleep(Duration::from_millis(5));
    }
}

/// One hourly line read back from a peer's output.
#[derive(Debug, PartialEq, Eq)]
struct Hour<'a> {
    day: u32,
    clock: &'a str,
    tick: u32,
    shared: Vec<(&'a str, &'a str)>,
}
fn field<'a>(line: &'a str, name: &str) -> Option<&'a str> {
    let key = format!("\"{name}\":");
    let rest = &line[line.find(&key)? + key.len()..];
    match rest.strip_prefix('"') {
        Some(text) => text.split('"').next(),
        None => rest.split([',', '}']).next(),
    }
}
fn hour(line: &str) -> Option<Hour<'_>> {
    let shared = &line[line.find("\"shared\":{")? + 10..];
    let shared = shared.strip_suffix("}}")?;
    let shared = shared
        .split(',')
        .filter(|pair| !pair.is_empty())
        .map(|pair| {
            let (tag, value) = pair.split_once(':')?;
            Some((tag.trim_matches('"'), value.trim_matches('"')))
        })
        .collect::<Option<Vec<_>>>()?;
    Some(Hour {
        day: field(line, "day")?.parse().ok()?,
        clock: field(line, "clock")?,
        tick: field(line, "tick")?.parse().ok()?,
        shared,
    })
}
fn hours(log: &str) -> Vec<Hour<'_>> {
    log.lines().filter_map(hour).collect()
}
/// The `{"end":…}` reason, if the peer got that far.
fn end_reason(log: &str) -> Option<&str> {
    log.lines()
        .rev()
        .find(|line| line.starts_with("{\"end\":"))
        .and_then(|line| field(line, "end"))
}

/// Compare a host's output with each client's, hour by hour: the same hours
/// at the same Total tick with the same digest for every shared tag, no
/// desync sample on the host, and a clean end on every peer. Returns the
/// summary line, or the first problem found.
pub fn compare_logs(host: &str, clients: &[&str]) -> Result<String, String> {
    let host_hours = hours(host);
    let (first, last) = match (host_hours.first(), host_hours.last()) {
        (Some(first), Some(last)) => (first, last),
        _ => return Err("the host printed no hourly lines".into()),
    };
    let mut ticks = Vec::new();
    for (index, client) in clients.iter().enumerate() {
        let name = format!("client {}", index + 1);
        let client_hours = hours(client);
        for expected in &host_hours {
            let Some(actual) = client_hours
                .iter()
                .find(|h| (h.day, h.clock) == (expected.day, expected.clock))
            else {
                return Err(format!(
                    "{name} has no line for day {} {}",
                    expected.day, expected.clock
                ));
            };
            let at = format!(
                "day {} {} (host tick {}, {name} tick {})",
                expected.day, expected.clock, expected.tick, actual.tick
            );
            if actual.tick != expected.tick {
                return Err(format!("first divergent hour: {at}: Total tick differs"));
            }
            if let Some(((tag, _), _)) = expected
                .shared
                .iter()
                .zip(&actual.shared)
                .find(|(left, right)| left != right)
            {
                return Err(format!("first divergent hour: {at}: shared tag {tag}"));
            }
            if expected.shared.len() != actual.shared.len() {
                return Err(format!(
                    "first divergent hour: {at}: shared tag lists differ"
                ));
            }
        }
        match end_reason(client) {
            Some("ticks") => (),
            Some(reason) => return Err(format!("{name} ended with {reason}")),
            None => return Err(format!("{name} did not finish")),
        }
        ticks.push(format!(
            "{name} tick {}",
            client_hours.last().map_or(0, |h| h.tick)
        ));
    }
    let samples: Vec<_> = host
        .lines()
        .filter(|line| line.starts_with("{\"sample\":"))
        .collect();
    if let Some(desync) = samples.iter().find(|line| line.contains("\"desync\"")) {
        return Err(format!("the host reported a desync: {desync}"));
    }
    match end_reason(host) {
        Some("ticks") => (),
        Some(reason) => return Err(format!("the host ended with {reason}")),
        None => return Err("the host did not finish".into()),
    }
    let matched = samples.iter().filter(|l| l.contains("\"match\"")).count();
    let days = (last.tick - first.tick) / super::TICKS_PER_DAY;
    Ok(format!(
        "{days} day{} in sync (host tick {}, {}; {matched} of {} samples compared and matched)",
        if days == 1 { "" } else { "s" },
        last.tick,
        ticks.join(", "),
        samples.len()
    ))
}

/// `game headless --compare HOST_LOG CLIENT_LOG…`: print `compare_logs`.
pub fn compare_main(paths: &[String]) -> i32 {
    let read = |path: &String| std::fs::read_to_string(path).map_err(|e| format!("{path}: {e}"));
    let logs = match paths.iter().map(read).collect::<Result<Vec<_>, _>>() {
        Ok(logs) if logs.len() >= 2 => logs,
        Ok(_) => {
            eprintln!("usage: game headless --compare HOST_LOG CLIENT_LOG...");
            return 2;
        }
        Err(error) => {
            eprintln!("{error}");
            return 2;
        }
    };
    let clients: Vec<&str> = logs[1..].iter().map(String::as_str).collect();
    match compare_logs(&logs[0], &clients) {
        Ok(summary) => {
            println!("{summary}");
            0
        }
        Err(problem) => {
            println!("{problem}");
            1
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use openmt2001::network::synchronization::{Field, Mismatch};

    #[test]
    fn hour_and_sample_lines_have_the_documented_shape() {
        let line = hour_line(2, 9, 15000, &[(2, b"abc".to_vec()), (18, vec![])]);
        assert_eq!(
            line,
            "{\"day\":2,\"clock\":\"09:00\",\"tick\":15000,\"shared\":{\"2\":\"ba7816bf8f01cfea\",\"18\":\"e3b0c44298fc1c14\"}}"
        );
        let mut comparison = Comparison {
            sender: 1,
            tick: 400,
            local: true,
            differences: vec![],
        };
        assert_eq!(
            sample_line(&comparison),
            "{\"sample\":400,\"player\":1,\"result\":\"match\"}"
        );
        comparison.differences.push(Mismatch {
            sender: 1,
            tick: 400,
            field: Field::People,
            local: 3,
            remote: 4,
        });
        assert_eq!(
            sample_line(&comparison),
            "{\"sample\":400,\"player\":1,\"result\":\"desync\",\"differences\":[{\"field\":\"People\",\"local\":3,\"remote\":4}]}"
        );
        comparison.local = false;
        assert!(sample_line(&comparison).ends_with("\"result\":\"no local sample\"}"));
    }

    #[test]
    fn started_roster_gives_compact_slots_and_remote_humans() {
        use openmt2001::network::{
            Row,
            lobby::{Slot, worker::View},
        };
        let mut snapshot = Snapshot::host(
            Settings {
                name: b"Headless".to_vec(),
                level: 300000,
                parameter_bits: 3000f32.to_bits(),
                difficulty: 1,
                map: b"empty.lua".to_vec(),
            },
            b"Host".to_vec(),
            4,
        )
        .unwrap();
        let row = |kind, token, peer| Slot {
            player: Row {
                kind,
                token,
                ready: true,
                name: b"Row".to_vec(),
            },
            peer,
            difficulty: 0,
        };
        snapshot.rows[1] = row(RowKind::Human, 0, Some(3));
        snapshot.rows[3] = row(RowKind::Ai, 1, None);
        snapshot.rows[4] = row(RowKind::Human, 2, Some(5));
        let ticket = |local_peer| Ticket {
            view: View {
                snapshot: snapshot.clone(),
                local_peer,
                host: local_peer == 0,
                address: "127.0.0.1:2300".parse().unwrap(),
            },
            seed: 7,
            expected: None,
        };
        let host = Peer::from_ticket(&ticket(0)).unwrap();
        assert_eq!((host.host, host.slot, host.seed), (true, 0, 7));
        assert_eq!(host.remote, [1, 3]);
        let client = Peer::from_ticket(&ticket(5)).unwrap();
        assert_eq!((client.host, client.slot), (false, 3));
        assert_eq!(client.remote, [0, 1]);
        assert!(Peer::from_ticket(&ticket(4)).is_err());
    }

    fn log(lines: &[String], end: &str) -> String {
        let mut text = String::from("{\"connected\":\"host\",\"slot\":0,\"seed\":1}\n");
        for line in lines {
            text += line;
            text += "\n";
        }
        text + &format!("{{\"end\":\"{end}\",\"ticks\":15000}}\n")
    }
    fn day(change: Option<(u32, u16)>) -> Vec<String> {
        (0..=24)
            .map(|h| {
                let tick = h * 625;
                let shared: Vec<_> = [2u16, 3, 18]
                    .into_iter()
                    .map(|tag| {
                        let mut bytes = vec![tag as u8; 4];
                        if change.is_some_and(|(at, t)| tick >= at && t == tag) {
                            bytes[0] ^= 1;
                        }
                        (tag, bytes)
                    })
                    .collect();
                hour_line(1 + (9 + h) / 24, (9 + h) % 24, tick, &shared)
            })
            .collect()
    }

    #[test]
    fn compare_reports_sync_or_the_first_divergent_hour_and_tag() {
        let sample = "{\"sample\":200,\"player\":1,\"result\":\"match\"}".to_owned();
        let mut host_lines = day(None);
        host_lines.insert(1, sample.clone());
        let host = log(&host_lines, "ticks");
        assert_eq!(
            compare_logs(&host, &[&log(&day(None), "ticks")]),
            Ok("1 day in sync (host tick 15000, client 1 tick 15000; 1 of 1 samples compared and matched)".into())
        );
        // Tag 3 diverges from 03:00 on day 2 (tick 11250).
        let divergent = log(&day(Some((11250, 3))), "ticks");
        assert_eq!(
            compare_logs(&host, &[&divergent]),
            Err("first divergent hour: day 2 03:00 (host tick 11250, client 1 tick 11250): shared tag 3".into())
        );
        let short = log(&day(None)[..10], "failure: Host connection ended");
        assert_eq!(
            compare_logs(&host, &[&short]),
            Err("client 1 has no line for day 1 19:00".into())
        );
        let stopped = log(&day(None), "failure: lost");
        assert_eq!(
            compare_logs(&host, &[&stopped]),
            Err("client 1 ended with failure: lost".into())
        );
        let mut late = day(None);
        late[4] = late[4].replace("\"tick\":2500", "\"tick\":2501");
        assert!(
            compare_logs(&host, &[&log(&late, "ticks")])
                .unwrap_err()
                .ends_with("day 1 13:00 (host tick 2500, client 1 tick 2501): Total tick differs")
        );
        let mut desync = host_lines.clone();
        desync
            .push("{\"sample\":400,\"player\":1,\"result\":\"desync\",\"differences\":[]}".into());
        assert!(
            compare_logs(
                &log(&desync, "desync 1 differences"),
                &[&log(&day(None), "ticks")]
            )
            .unwrap_err()
            .starts_with("the host reported a desync")
        );
        assert!(compare_logs("", &[&host]).is_err());
    }

    /// A short idle Daily Profit game on empty.lua, both peers in this
    /// process on their own transport owners, through the real lobby, TCP
    /// transport, connected start and frame path.
    #[test]
    #[ignore = "requires original assets; two connected cities over loopback TCP"]
    fn in_process_connected_soak_stays_in_sync() {
        let port = std::net::TcpListener::bind("127.0.0.1:0")
            .unwrap()
            .local_addr()
            .unwrap()
            .port();
        let options = |start| Options {
            start,
            paths: super::super::Paths::from_fixtures(None, None).unwrap(),
            ticks: 1200,
            every: None,
            frame_ms: 40,
            dialogs: super::super::DialogPolicy::Last,
            auctions: super::super::AuctionPolicy::Pass,
            write_save: None,
            autosave_dir: None,
            autosaves: None,
            rng: false,
            checkpoint_saves: None,
            shared_dir: None,
            pace_ms: 0,
        };
        let host = options(Start::Host {
            port,
            level: "net0".into(),
            map: "empty.lua".into(),
            target: None,
            difficulty: 1,
            character: 4,
            players: 2,
            ai: 0,
        });
        let client = options(Start::Join {
            address: SocketAddr::from(([127, 0, 0, 1], port)),
            character: 3,
        });
        let peer = |options: Options| {
            std::thread::Builder::new()
                .stack_size(64 * 1024 * 1024)
                .spawn(move || {
                    ISOLATED_OWNER.set(true);
                    let mut out = Vec::new();
                    let end = super::super::run(&options, &mut out);
                    (end, String::from_utf8(out).unwrap())
                })
                .unwrap()
        };
        let (host, client) = (peer(host), peer(client));
        let (host_end, host_log) = host.join().unwrap();
        let (client_end, client_log) = client.join().unwrap();
        assert_eq!(host_end, Ok(End::Ticks), "{host_log}");
        assert_eq!(client_end, Ok(End::Ticks), "{client_log}");
        let summary = compare_logs(&host_log, &[&client_log]).unwrap();
        assert!(
            summary.contains("host tick 1200, client 1 tick 1200"),
            "{summary}"
        );
        assert!(host_log.contains("{\"sample\":1200,\"player\":1,\"result\":\"match\"}"));
    }
}
