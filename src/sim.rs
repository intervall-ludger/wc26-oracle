use crate::bracket::*;
use crate::model::Data;
use rand::Rng;
use rand_distr::{Distribution, Poisson};

const MIN_LAMBDA: f64 = 0.18;

#[derive(Clone, Copy)]
pub struct Config {
    pub home_adv: f64,    // elo bonus for host nations
    pub total_goals: f64, // expected goals summed over both teams
    pub supremacy: f64,   // goal-diff per elo point
    pub form_weight: f64, // multiplier on each team's form value
    pub dc_rho: f64,      // Dixon-Coles low-score correlation (typically slightly negative)
    pub dyn_k: f64,       // in-tournament dynamic-Elo K-factor (0 disables)
}

impl Default for Config {
    fn default() -> Self {
        // Calibrated so the title spread matches the Opta supercomputer reasonably well.
        Config {
            home_adv: 70.0,
            total_goals: 2.6,
            supremacy: 0.0028,
            form_weight: 1.0,
            dc_rho: -0.10,
            dyn_k: 8.0,
        }
    }
}

/// Dixon-Coles correction for the four low-scoring cells; 1.0 elsewhere.
fn dc_tau(i: u32, j: u32, la: f64, lb: f64, rho: f64) -> f64 {
    match (i, j) {
        (0, 0) => 1.0 - la * lb * rho,
        (0, 1) => 1.0 + la * rho,
        (1, 0) => 1.0 + lb * rho,
        (1, 1) => 1.0 - rho,
        _ => 1.0,
    }
}

pub struct Tally {
    pub sims: u64,
    pub n: usize,
    pub group_winner: Vec<u64>,
    pub runner_up: Vec<u64>,
    pub third_through: Vec<u64>,
    pub reach_r16: Vec<u64>,
    pub reach_qf: Vec<u64>,
    pub reach_sf: Vec<u64>,
    pub reach_final: Vec<u64>,
    pub champion: Vec<u64>,
    /// Per team: how often it finished 1st/2nd/3rd/4th in its group, and total points (for the average).
    pub group_pos: Vec<[u64; 4]>,
    pub points_sum: Vec<u64>,
    /// Per knockout match: how often each team appeared on side A / side B (R32 only) and won it.
    pub slot_a: std::collections::HashMap<u32, Vec<u64>>,
    pub slot_b: std::collections::HashMap<u32, Vec<u64>>,
    pub win: std::collections::HashMap<u32, Vec<u64>>,
    /// opp[team][round 0=R32..4=Final][opponent] = times faced. Drives the path explorer.
    pub opp: Vec<[Vec<u64>; 5]>,
}

impl Tally {
    pub fn new(n: usize) -> Self {
        let z = || vec![0u64; n];
        let (mut slot_a, mut slot_b, mut win) = (
            std::collections::HashMap::new(),
            std::collections::HashMap::new(),
            std::collections::HashMap::new(),
        );
        for km in knockout() {
            win.insert(km.id, z());
            if km.round == Round::R32 {
                slot_a.insert(km.id, z());
                slot_b.insert(km.id, z());
            }
        }
        Tally {
            sims: 0,
            n,
            group_winner: z(),
            runner_up: z(),
            third_through: z(),
            reach_r16: z(),
            reach_qf: z(),
            reach_sf: z(),
            reach_final: z(),
            champion: z(),
            group_pos: vec![[0u64; 4]; n],
            points_sum: z(),
            slot_a,
            slot_b,
            win,
            opp: (0..n).map(|_| [z(), z(), z(), z(), z()]).collect(),
        }
    }
}

/// Team strength: base Elo + form + host bonus + (optional) in-tournament dynamic delta.
fn elo_adj(data: &Data, cfg: &Config, dynv: &[f64], team: usize) -> f64 {
    let t = &data.teams[team];
    let dynamic = dynv.get(team).copied().unwrap_or(0.0);
    t.elo
        + cfg.form_weight * t.form
        + dynamic
        + if data.is_host(team) {
            cfg.home_adv
        } else {
            0.0
        }
}

/// Expected goals (lambda) for both sides of a fixture.
fn lambdas(data: &Data, cfg: &Config, dynv: &[f64], a: usize, b: usize) -> (f64, f64) {
    let d = elo_adj(data, cfg, dynv, a) - elo_adj(data, cfg, dynv, b);
    let sup = d * cfg.supremacy;
    let la = ((cfg.total_goals + sup) / 2.0).max(MIN_LAMBDA);
    let lb = ((cfg.total_goals - sup) / 2.0).max(MIN_LAMBDA);
    (la, lb)
}

/// Update both teams' dynamic-Elo deltas after a match (Elo step scaled by goal margin).
fn bump_dyn(data: &Data, cfg: &Config, dynv: &mut [f64], a: usize, b: usize, ga: u32, gb: u32) {
    if cfg.dyn_k <= 0.0 || dynv.is_empty() {
        return;
    }
    let we = 1.0
        / (1.0 + 10f64.powf((elo_adj(data, cfg, dynv, b) - elo_adj(data, cfg, dynv, a)) / 400.0));
    let s = match ga.cmp(&gb) {
        std::cmp::Ordering::Greater => 1.0,
        std::cmp::Ordering::Less => 0.0,
        std::cmp::Ordering::Equal => 0.5,
    };
    let margin = (1.0 + (ga as i32 - gb as i32).unsigned_abs() as f64).ln();
    let delta = cfg.dyn_k * margin * (s - we);
    dynv[a] = (dynv[a] + delta).clamp(-120.0, 120.0);
    dynv[b] = (dynv[b] - delta).clamp(-120.0, 120.0);
}

/// Analytic pre-match probabilities from the Poisson model with Dixon-Coles correction.
pub struct Odds {
    pub home: f64,
    pub draw: f64,
    pub away: f64,
}

pub fn match_odds(data: &Data, cfg: &Config, a: usize, b: usize) -> Odds {
    let (la, lb) = lambdas(data, cfg, &[], a, b);
    let pmf = |lambda: f64, k: i32| (-lambda).exp() * lambda.powi(k) / factorial(k);
    let max = 12;
    let (mut ph, mut pd, mut pa) = (0.0, 0.0, 0.0);
    for i in 0..=max {
        for j in 0..=max {
            let p = pmf(la, i) * pmf(lb, j) * dc_tau(i as u32, j as u32, la, lb, cfg.dc_rho);
            match i.cmp(&j) {
                std::cmp::Ordering::Greater => ph += p,
                std::cmp::Ordering::Less => pa += p,
                std::cmp::Ordering::Equal => pd += p,
            }
        }
    }
    let z = ph + pd + pa;
    Odds {
        home: ph / z,
        draw: pd / z,
        away: pa / z,
    }
}

fn factorial(k: i32) -> f64 {
    (1..=k).fold(1.0, |acc, x| acc * x as f64)
}

/// Expected goals for both sides (the model's headline prediction for a fixture).
pub fn expected_goals(data: &Data, cfg: &Config, a: usize, b: usize) -> (f64, f64) {
    lambdas(data, cfg, &[], a, b)
}

/// Sample (or look up actual) score, Dixon-Coles adjusted via rejection sampling.
fn play(
    data: &Data,
    cfg: &Config,
    dynv: &[f64],
    a: usize,
    b: usize,
    id: &str,
    rng: &mut impl Rng,
) -> (u32, u32) {
    if let Some(r) = data.results.get(id) {
        return (r.home, r.away);
    }
    let (la, lb) = lambdas(data, cfg, dynv, a, b);
    let pa = Poisson::new(la).unwrap();
    let pb = Poisson::new(lb).unwrap();
    if cfg.dc_rho == 0.0 {
        return (pa.sample(rng) as u32, pb.sample(rng) as u32);
    }
    let m = [
        dc_tau(0, 0, la, lb, cfg.dc_rho),
        dc_tau(0, 1, la, lb, cfg.dc_rho),
        dc_tau(1, 0, la, lb, cfg.dc_rho),
        dc_tau(1, 1, la, lb, cfg.dc_rho),
        1.0,
    ]
    .into_iter()
    .fold(1.0_f64, f64::max);
    loop {
        let ga = pa.sample(rng) as u32;
        let gb = pb.sample(rng) as u32;
        if rng.gen::<f64>() < dc_tau(ga, gb, la, lb, cfg.dc_rho) / m {
            return (ga, gb);
        }
    }
}

/// Knockout: winner + score; draws settled by an elo-weighted shootout.
fn play_ko(
    data: &Data,
    cfg: &Config,
    dynv: &[f64],
    a: usize,
    b: usize,
    id: &str,
    rng: &mut impl Rng,
) -> (usize, u32, u32) {
    if let Some(r) = data.results.get(id) {
        let w = if r.home != r.away {
            if r.home > r.away {
                a
            } else {
                b
            }
        } else {
            match r.winner.as_deref() {
                Some("away") => b,
                _ => a,
            }
        };
        return (w, r.home, r.away);
    }
    let (ga, gb) = play(data, cfg, dynv, a, b, id, rng);
    if ga != gb {
        return (if ga > gb { a } else { b }, ga, gb);
    }
    let we = 1.0
        / (1.0 + 10f64.powf((elo_adj(data, cfg, dynv, b) - elo_adj(data, cfg, dynv, a)) / 400.0));
    (if rng.gen::<f64>() < we { a } else { b }, ga, gb)
}

struct Standing {
    team: usize,
    pts: u32,
    gd: i32,
    gf: i32,
    tie: f64,
}

fn simulate_group(
    data: &Data,
    cfg: &Config,
    dynv: &mut Vec<f64>,
    g: char,
    rng: &mut impl Rng,
) -> Vec<Standing> {
    let teams = data.group_teams(g);
    let mut pts = [0u32; 4];
    let mut gf = [0i32; 4];
    let mut ga = [0i32; 4];
    let mut scored = [[0i32; 4]; 4];

    for &(hp, ap) in GROUP_FIXTURES.iter() {
        let (hi, ai) = (hp - 1, ap - 1);
        let id = group_match_id(g, hp, ap);
        let (gh, gv) = play(data, cfg, dynv, teams[hi], teams[ai], &id, rng);
        bump_dyn(data, cfg, dynv, teams[hi], teams[ai], gh, gv);
        gf[hi] += gh as i32;
        ga[hi] += gv as i32;
        gf[ai] += gv as i32;
        ga[ai] += gh as i32;
        scored[hi][ai] = gh as i32;
        scored[ai][hi] = gv as i32;
        match gh.cmp(&gv) {
            std::cmp::Ordering::Greater => pts[hi] += 3,
            std::cmp::Ordering::Less => pts[ai] += 3,
            std::cmp::Ordering::Equal => {
                pts[hi] += 1;
                pts[ai] += 1;
            }
        }
    }

    let elo = [
        data.teams[teams[0]].elo,
        data.teams[teams[1]].elo,
        data.teams[teams[2]].elo,
        data.teams[teams[3]].elo,
    ];
    // only used if two teams share identical Elo; random keeps such ties unbiased across sims
    let last_resort = [rng.gen(), rng.gen(), rng.gen(), rng.gen()];
    rank_group(&pts, &gf, &ga, &scored, &elo, &last_resort)
        .into_iter()
        .map(|i| Standing {
            team: teams[i],
            pts: pts[i],
            gd: gf[i] - ga[i],
            gf: gf[i],
            tie: 0.0,
        })
        .collect()
}

/// Rank the four teams of a group by the FIFA 2026 criteria: total points, then head-to-head
/// (points, goal difference, goals among the teams still level), then overall goal difference,
/// overall goals, FIFA World Ranking (approximated by Elo). Fair play is skipped (no card data).
/// `last_resort` only separates teams with identical Elo, which the real rules never reach.
/// Returns the local indices 0..4 best to worst.
fn rank_group(
    pts: &[u32; 4],
    gf: &[i32; 4],
    ga: &[i32; 4],
    scored: &[[i32; 4]; 4],
    elo: &[f64; 4],
    last_resort: &[f64; 4],
) -> Vec<usize> {
    let mut idx = [0usize, 1, 2, 3];
    idx.sort_by(|&a, &b| pts[b].cmp(&pts[a]));
    let mut out = Vec::with_capacity(4);
    let mut i = 0;
    while i < 4 {
        let mut j = i + 1;
        while j < 4 && pts[idx[j]] == pts[idx[i]] {
            j += 1;
        }
        resolve_block(&idx[i..j], gf, ga, scored, elo, last_resort, &mut out);
        i = j;
    }
    out
}

/// Order a block of teams that are all level on total points. Applies the head-to-head mini-table
/// among the block; teams it cannot separate fall through to overall goal difference, overall
/// goals, World Ranking (Elo) and finally `last_resort`. The head-to-head is re-applied to any
/// still-tied sub-block (FIFA's recursive criterion).
fn resolve_block(
    block: &[usize],
    gf: &[i32; 4],
    ga: &[i32; 4],
    scored: &[[i32; 4]; 4],
    elo: &[f64; 4],
    last_resort: &[f64; 4],
    out: &mut Vec<usize>,
) {
    if block.len() == 1 {
        out.push(block[0]);
        return;
    }
    // head-to-head key (points, goal diff, goals) counting only matches within the block
    let h2h = |t: usize| -> (i32, i32, i32) {
        let (mut p, mut sf, mut sa) = (0, 0, 0);
        for &o in block {
            if o != t {
                let (a, b) = (scored[t][o], scored[o][t]);
                sf += a;
                sa += b;
                p += match a.cmp(&b) {
                    std::cmp::Ordering::Greater => 3,
                    std::cmp::Ordering::Equal => 1,
                    std::cmp::Ordering::Less => 0,
                };
            }
        }
        (p, sf - sa, sf)
    };
    let mut order = block.to_vec();
    order.sort_by(|&a, &b| h2h(b).cmp(&h2h(a)));
    let mut i = 0;
    while i < order.len() {
        let mut j = i + 1;
        while j < order.len() && h2h(order[j]) == h2h(order[i]) {
            j += 1;
        }
        if j - i == block.len() {
            // head-to-head separated nothing; fall through to the remaining FIFA criteria
            let mut tail = order[i..j].to_vec();
            tail.sort_by(|&a, &b| {
                (gf[b] - ga[b])
                    .cmp(&(gf[a] - ga[a]))
                    .then(gf[b].cmp(&gf[a]))
                    .then(elo[b].partial_cmp(&elo[a]).unwrap())
                    .then(last_resort[b].partial_cmp(&last_resort[a]).unwrap())
            });
            out.extend(tail);
        } else {
            resolve_block(&order[i..j], gf, ga, scored, elo, last_resort, out);
        }
        i = j;
    }
}

/// Final group order from real results only. None if not all 6 games are played.
/// Returns the four team indices ranked 1st..4th. Ties: points, GD, GF, then draw position.
pub fn real_group_standings(data: &Data, g: char) -> Option<[usize; 4]> {
    let teams = data.group_teams(g);
    let mut pts = [0u32; 4];
    let mut gf = [0i32; 4];
    let mut ga = [0i32; 4];
    let mut scored = [[0i32; 4]; 4];
    for &(hp, ap) in GROUP_FIXTURES.iter() {
        let id = group_match_id(g, hp, ap);
        let r = data.results.get(&id)?;
        let (hi, ai) = (hp - 1, ap - 1);
        gf[hi] += r.home as i32;
        ga[hi] += r.away as i32;
        gf[ai] += r.away as i32;
        ga[ai] += r.home as i32;
        scored[hi][ai] = r.home as i32;
        scored[ai][hi] = r.away as i32;
        match r.home.cmp(&r.away) {
            std::cmp::Ordering::Greater => pts[hi] += 3,
            std::cmp::Ordering::Less => pts[ai] += 3,
            std::cmp::Ordering::Equal => {
                pts[hi] += 1;
                pts[ai] += 1;
            }
        }
    }
    let elo = [
        data.teams[teams[0]].elo,
        data.teams[teams[1]].elo,
        data.teams[teams[2]].elo,
        data.teams[teams[3]].elo,
    ];
    // locked table stays deterministic: on identical Elo, the lower draw position ranks higher
    let last_resort = [-0.0, -1.0, -2.0, -3.0];
    let order = rank_group(&pts, &gf, &ga, &scored, &elo, &last_resort);
    Some([
        teams[order[0]],
        teams[order[1]],
        teams[order[2]],
        teams[order[3]],
    ])
}

/// The 8 best third-placed groups (in ranking order) from real results.
/// None unless all 12 groups are complete.
pub fn real_thirds_order(data: &Data) -> Option<Vec<char>> {
    let mut thirds: Vec<(char, u32, i32, i32, f64)> = Vec::with_capacity(12);
    for g in all_groups() {
        let teams = data.group_teams(g);
        let mut pts = [0u32; 4];
        let mut gf = [0i32; 4];
        let mut ga = [0i32; 4];
        for &(hp, ap) in GROUP_FIXTURES.iter() {
            let r = data.results.get(&group_match_id(g, hp, ap))?;
            let (hi, ai) = (hp - 1, ap - 1);
            gf[hi] += r.home as i32;
            ga[hi] += r.away as i32;
            gf[ai] += r.away as i32;
            ga[ai] += r.home as i32;
            match r.home.cmp(&r.away) {
                std::cmp::Ordering::Greater => pts[hi] += 3,
                std::cmp::Ordering::Less => pts[ai] += 3,
                std::cmp::Ordering::Equal => {
                    pts[hi] += 1;
                    pts[ai] += 1;
                }
            }
        }
        let third = real_group_standings(data, g)?[2];
        let i = teams.iter().position(|&t| t == third).unwrap();
        thirds.push((g, pts[i], gf[i] - ga[i], gf[i], data.teams[third].elo));
    }
    // third-placed ranking: points, goal diff, goals, world ranking (Elo), then group letter
    thirds.sort_by(|a, b| {
        b.1.cmp(&a.1)
            .then(b.2.cmp(&a.2))
            .then(b.3.cmp(&a.3))
            .then(b.4.partial_cmp(&a.4).unwrap())
            .then(a.0.cmp(&b.0))
    });
    thirds.truncate(8);
    Some(thirds.into_iter().map(|t| t.0).collect())
}

pub fn simulate_tournament(data: &Data, cfg: &Config, tally: &mut Tally, rng: &mut impl Rng) {
    use std::collections::HashMap;

    let mut winners: HashMap<char, usize> = HashMap::new();
    let mut runners: HashMap<char, usize> = HashMap::new();
    let mut thirds: Vec<Standing> = Vec::with_capacity(12);
    // per-tournament dynamic-Elo deltas (empty if dynamic Elo is disabled)
    let mut dynv: Vec<f64> = if cfg.dyn_k > 0.0 {
        vec![0.0; data.teams.len()]
    } else {
        Vec::new()
    };

    for g in all_groups() {
        let table = simulate_group(data, cfg, &mut dynv, g, rng);
        winners.insert(g, table[0].team);
        runners.insert(g, table[1].team);
        tally.group_winner[table[0].team] += 1;
        tally.runner_up[table[1].team] += 1;
        for (k, st) in table.iter().enumerate() {
            tally.group_pos[st.team][k] += 1;
            tally.points_sum[st.team] += st.pts as u64;
        }
        thirds.push(Standing {
            team: table[2].team,
            pts: table[2].pts,
            gd: table[2].gd,
            gf: table[2].gf,
            tie: rng.gen::<f64>(),
        });
    }

    // rank the 12 third-placed teams, take the best 8: points, goal diff, goals, World Ranking
    // (Elo proxy); fair play is skipped, the random `tie` only splits identical-Elo teams
    thirds.sort_by(|x, y| {
        y.pts
            .cmp(&x.pts)
            .then(y.gd.cmp(&x.gd))
            .then(y.gf.cmp(&x.gf))
            .then(
                data.teams[y.team]
                    .elo
                    .partial_cmp(&data.teams[x.team].elo)
                    .unwrap(),
            )
            .then(y.tie.partial_cmp(&x.tie).unwrap())
    });
    thirds.truncate(8);
    let third_group_of: HashMap<char, usize> = thirds
        .iter()
        .map(|s| (data.teams[s.team].group.chars().next().unwrap(), s.team))
        .collect();
    for s in &thirds {
        tally.third_through[s.team] += 1;
    }

    let third_groups: Vec<char> = third_group_of.keys().copied().collect();
    let assigned = crate::third_table::assign_thirds(&third_groups);
    let third_team_for_match: HashMap<u32, usize> = assigned
        .into_iter()
        .map(|(mid, g)| (mid, third_group_of[&g]))
        .collect();

    let resolve = |src: &Source, won: &HashMap<u32, usize>| -> usize {
        match src {
            Source::Winner(g) => winners[g],
            Source::RunnerUp(g) => runners[g],
            Source::MatchWinner(n) => won[n],
            Source::Third => unreachable!("third resolved per match id"),
        }
    };

    let mut won: HashMap<u32, usize> = HashMap::new();
    for km in knockout() {
        let a = match km.a {
            Source::Third => third_team_for_match[&km.id],
            ref s => resolve(s, &won),
        };
        let b = match km.b {
            Source::Third => third_team_for_match[&km.id],
            ref s => resolve(s, &won),
        };
        let id = format!("M{}", km.id);
        let (w, gh, gv) = play_ko(data, cfg, &dynv, a, b, &id, rng);
        bump_dyn(data, cfg, &mut dynv, a, b, gh, gv);
        won.insert(km.id, w);

        let r = match km.round {
            Round::R32 => 0,
            Round::R16 => 1,
            Round::Qf => 2,
            Round::Sf => 3,
            Round::Final => 4,
        };
        tally.opp[a][r][b] += 1;
        tally.opp[b][r][a] += 1;

        if km.round == Round::R32 {
            tally.slot_a.get_mut(&km.id).unwrap()[a] += 1;
            tally.slot_b.get_mut(&km.id).unwrap()[b] += 1;
        }
        tally.win.get_mut(&km.id).unwrap()[w] += 1;

        match km.round {
            Round::R32 => tally.reach_r16[w] += 1,
            Round::R16 => tally.reach_qf[w] += 1,
            Round::Qf => tally.reach_sf[w] += 1,
            Round::Sf => tally.reach_final[w] += 1,
            Round::Final => tally.champion[w] += 1,
        }
    }

    tally.sims += 1;
}
