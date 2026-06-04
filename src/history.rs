use crate::model::Data;
use crate::sim::Tally;
use serde::{Deserialize, Serialize};
use std::collections::hash_map::DefaultHasher;
use std::collections::HashMap;
use std::hash::{Hash, Hasher};

#[derive(Serialize, Deserialize, Clone)]
pub struct Snapshot {
    pub sig: String,
    pub label: String,
    pub results: usize,
    pub champion: HashMap<String, f64>,
    pub advance: HashMap<String, f64>,
}

#[derive(Serialize, Deserialize, Default)]
pub struct History {
    pub snapshots: Vec<Snapshot>,
}

impl History {
    pub fn load(path: &str) -> Self {
        std::fs::read_to_string(path)
            .ok()
            .and_then(|raw| serde_json::from_str(&raw).ok())
            .unwrap_or_default()
    }

    pub fn save(&self, path: &str) {
        if let Ok(json) = serde_json::to_string_pretty(self) {
            let _ = std::fs::write(path, json);
        }
    }

    /// Insert or replace the snapshot for the current state, keeping chronological order.
    pub fn upsert(&mut self, snap: Snapshot) {
        match self.snapshots.iter_mut().find(|s| s.sig == snap.sig) {
            Some(existing) => *existing = snap,
            None => self.snapshots.push(snap),
        }
        self.snapshots.sort_by_key(|s| s.results);
    }
}

fn pct(n: u64, total: u64) -> f64 {
    if total == 0 { 0.0 } else { 100.0 * n as f64 / total as f64 }
}

/// Stable signature of the entered results (count + hash of id/score pairs).
fn signature(data: &Data) -> String {
    let mut ids: Vec<String> = data
        .results
        .iter()
        .map(|(id, r)| format!("{id}={}-{}-{}", r.home, r.away, r.winner.as_deref().unwrap_or("")))
        .collect();
    ids.sort();
    let mut h = DefaultHasher::new();
    ids.hash(&mut h);
    format!("{}:{:x}", data.results.len(), h.finish())
}

/// Short timeline label for the current state.
pub fn stage_label(data: &Data) -> String {
    let group_played = data.results.keys().filter(|k| k.contains(':')).count();
    let ko: Vec<u32> = data
        .results
        .keys()
        .filter_map(|k| k.strip_prefix('M').and_then(|n| n.parse().ok()))
        .collect();

    if let Some(&max) = ko.iter().max() {
        return match max {
            73..=88 => "R32",
            89..=96 => "R16",
            97..=100 => "QF",
            101..=102 => "SF",
            _ => "Final",
        }
        .to_string();
    }
    if group_played == 0 {
        return "Pre".to_string();
    }
    if group_played >= 72 {
        return "Groups".to_string();
    }
    format!("MD{}", ((group_played - 1) / 24 + 1).min(3))
}

pub fn snapshot(data: &Data, t: &Tally) -> Snapshot {
    let champion = (0..t.n)
        .map(|i| (data.teams[i].code.clone(), pct(t.champion[i], t.sims)))
        .collect();
    let advance = (0..t.n)
        .map(|i| {
            (
                data.teams[i].code.clone(),
                pct(t.group_winner[i] + t.runner_up[i] + t.third_through[i], t.sims),
            )
        })
        .collect();
    Snapshot {
        sig: signature(data),
        label: stage_label(data),
        results: data.results.len(),
        champion,
        advance,
    }
}
