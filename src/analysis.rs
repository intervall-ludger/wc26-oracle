use crate::bracket::{group_match_id, knockout, third_slots, GROUP_FIXTURES, Source};
use crate::model::Data;
use crate::sim::{match_odds, match_thirds, real_group_standings, real_thirds_order, Config, Odds};
use std::collections::HashMap;

pub struct Played {
    pub stage: String,
    pub home: usize,
    pub away: usize,
    pub gh: u32,
    pub ga: u32,
    pub odds: Odds,
}

impl Played {
    /// Model probability assigned to the outcome that actually happened.
    pub fn prob_actual(&self) -> f64 {
        match self.gh.cmp(&self.ga) {
            std::cmp::Ordering::Greater => self.odds.home,
            std::cmp::Ordering::Less => self.odds.away,
            std::cmp::Ordering::Equal => self.odds.draw,
        }
    }

    /// Multiclass Brier score contribution (0 = perfect, 2 = worst).
    pub fn brier(&self) -> f64 {
        let (oh, od, oa) = match self.gh.cmp(&self.ga) {
            std::cmp::Ordering::Greater => (1.0, 0.0, 0.0),
            std::cmp::Ordering::Less => (0.0, 0.0, 1.0),
            std::cmp::Ordering::Equal => (0.0, 1.0, 0.0),
        };
        (self.odds.home - oh).powi(2) + (self.odds.draw - od).powi(2) + (self.odds.away - oa).powi(2)
    }
}

/// All matches that have a real result, with the model's pre-match odds for each.
pub fn played_matches(data: &Data, cfg: &Config) -> Vec<Played> {
    let mut out = Vec::new();

    // Group stage: participants are always known.
    for g in crate::bracket::all_groups() {
        let teams = data.group_teams(g);
        for (md, chunk) in GROUP_FIXTURES.chunks(2).enumerate() {
            for &(hp, ap) in chunk {
                let id = group_match_id(g, hp, ap);
                if let Some(r) = data.results.get(&id) {
                    let (home, away) = (teams[hp - 1], teams[ap - 1]);
                    out.push(Played {
                        stage: format!("Group {g} · MD{}", md + 1),
                        home,
                        away,
                        gh: r.home,
                        ga: r.away,
                        odds: match_odds(data, cfg, home, away),
                    });
                }
            }
        }
    }

    // Knockout: resolve participants from real results only.
    let winners: HashMap<char, usize> = crate::bracket::all_groups()
        .into_iter()
        .filter_map(|g| real_group_standings(data, g).map(|s| (g, s[0])))
        .collect();
    let runners: HashMap<char, usize> = crate::bracket::all_groups()
        .into_iter()
        .filter_map(|g| real_group_standings(data, g).map(|s| (g, s[1])))
        .collect();

    let third_team_for_match: HashMap<u32, usize> = match real_thirds_order(data) {
        Some(order) => {
            let assigned = match_thirds(&order, &third_slots());
            let third_idx: HashMap<char, usize> = order
                .iter()
                .map(|&g| (g, real_group_standings(data, g).unwrap()[2]))
                .collect();
            assigned.into_iter().map(|(mid, g)| (mid, third_idx[&g])).collect()
        }
        None => HashMap::new(),
    };

    let stage_name = |id: u32| match id {
        73..=88 => "Round of 32",
        89..=96 => "Round of 16",
        97..=100 => "Quarterfinal",
        101..=102 => "Semifinal",
        _ => "Final",
    };

    let mut won: HashMap<u32, usize> = HashMap::new();
    for km in knockout() {
        let resolve = |s: &Source| -> Option<usize> {
            match s {
                Source::Winner(g) => winners.get(g).copied(),
                Source::RunnerUp(g) => runners.get(g).copied(),
                Source::MatchWinner(n) => won.get(n).copied(),
                Source::Third(_) => third_team_for_match.get(&km.id).copied(),
            }
        };
        let (Some(home), Some(away)) = (resolve(&km.a), resolve(&km.b)) else {
            continue;
        };
        let id = format!("M{}", km.id);
        if let Some(r) = data.results.get(&id) {
            let w = if r.home != r.away {
                if r.home > r.away { home } else { away }
            } else {
                match r.winner.as_deref() {
                    Some("away") => away,
                    _ => home,
                }
            };
            won.insert(km.id, w);
            out.push(Played {
                stage: stage_name(km.id).to_string(),
                home,
                away,
                gh: r.home,
                ga: r.away,
                odds: match_odds(data, cfg, home, away),
            });
        }
    }

    out
}

/// Every fixture whose participants are determined: all group matches, plus knockout matches
/// once their feeders are decided by real results. Returns (match_id, home_idx, away_idx).
pub fn determined_matches(data: &Data) -> Vec<(String, usize, usize)> {
    use crate::bracket::all_groups;
    let mut out: Vec<(String, usize, usize)> = Vec::new();

    for g in all_groups() {
        let teams = data.group_teams(g);
        for &(hp, ap) in GROUP_FIXTURES.iter() {
            out.push((group_match_id(g, hp, ap), teams[hp - 1], teams[ap - 1]));
        }
    }

    let winners: HashMap<char, usize> = all_groups()
        .into_iter()
        .filter_map(|g| real_group_standings(data, g).map(|s| (g, s[0])))
        .collect();
    let runners: HashMap<char, usize> = all_groups()
        .into_iter()
        .filter_map(|g| real_group_standings(data, g).map(|s| (g, s[1])))
        .collect();
    let third_team_for_match: HashMap<u32, usize> = match real_thirds_order(data) {
        Some(order) => {
            let assigned = match_thirds(&order, &third_slots());
            let third_idx: HashMap<char, usize> = order
                .iter()
                .map(|&g| (g, real_group_standings(data, g).unwrap()[2]))
                .collect();
            assigned.into_iter().map(|(mid, g)| (mid, third_idx[&g])).collect()
        }
        None => HashMap::new(),
    };

    let mut won: HashMap<u32, usize> = HashMap::new();
    for km in knockout() {
        let resolve = |s: &Source| -> Option<usize> {
            match s {
                Source::Winner(g) => winners.get(g).copied(),
                Source::RunnerUp(g) => runners.get(g).copied(),
                Source::MatchWinner(n) => won.get(n).copied(),
                Source::Third(_) => third_team_for_match.get(&km.id).copied(),
            }
        };
        let (Some(home), Some(away)) = (resolve(&km.a), resolve(&km.b)) else {
            continue;
        };
        let id = format!("M{}", km.id);
        if let Some(r) = data.results.get(&id) {
            let w = if r.home != r.away {
                if r.home > r.away { home } else { away }
            } else {
                match r.winner.as_deref() {
                    Some("away") => away,
                    _ => home,
                }
            };
            won.insert(km.id, w);
        }
        out.push((id, home, away));
    }
    out
}

pub struct Accuracy {
    pub played: usize,
    pub model_brier: f64,
    pub baseline_brier: f64,
    pub correct_pick: usize,
}

/// How well the model called the matches that have actually been played.
pub fn accuracy(matches: &[Played]) -> Option<Accuracy> {
    if matches.is_empty() {
        return None;
    }
    let n = matches.len() as f64;
    let model_brier = matches.iter().map(|m| m.brier()).sum::<f64>() / n;
    // baseline: a uniform 1/3-1/3-1/3 guess scores exactly 2/3 per match
    let baseline_brier = 2.0 / 3.0;
    let correct_pick = matches
        .iter()
        .filter(|m| {
            let pick_home = m.odds.home >= m.odds.away && m.odds.home >= m.odds.draw;
            let pick_away = m.odds.away >= m.odds.home && m.odds.away >= m.odds.draw;
            (m.gh > m.ga && pick_home) || (m.ga > m.gh && pick_away)
        })
        .count();
    Some(Accuracy {
        played: matches.len(),
        model_brier,
        baseline_brier,
        correct_pick,
    })
}
