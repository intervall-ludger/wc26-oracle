use serde::Deserialize;
use std::collections::HashMap;

pub const GROUPS: [char; 12] = ['A', 'B', 'C', 'D', 'E', 'F', 'G', 'H', 'I', 'J', 'K', 'L'];

#[derive(Debug, Clone, Deserialize)]
pub struct Team {
    pub name: String,
    pub code: String,
    pub group: String,
    pub pos: usize,
    pub elo: f64,
    /// Recent-form adjustment in Elo points (positive = hot, negative = slump). Default 0.
    #[serde(default)]
    pub form: f64,
}

#[derive(Debug, Deserialize)]
pub struct TeamsFile {
    pub hosts: Vec<String>,
    pub teams: Vec<Team>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Result {
    #[serde(rename = "match")]
    pub match_id: String,
    pub home: u32,
    pub away: u32,
    #[serde(default)]
    pub winner: Option<String>,
    /// How a knockout tie was decided: "90", "aet", or "pens". Group games are always 90.
    #[serde(default)]
    pub decided: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct ResultsFile {
    #[serde(default)]
    pub results: Vec<Result>,
}

pub struct Data {
    pub teams: Vec<Team>,
    pub hosts: Vec<String>,
    pub results: HashMap<String, Result>,
    pub group_index: HashMap<char, Vec<usize>>,
}

impl Data {
    pub fn load(teams_path: &str, results_path: &str) -> Self {
        let raw = std::fs::read_to_string(teams_path)
            .unwrap_or_else(|e| panic!("cannot read {teams_path}: {e}"));
        let tf: TeamsFile = serde_json::from_str(&raw)
            .unwrap_or_else(|e| panic!("invalid {teams_path}: {e}"));

        let results: HashMap<String, Result> = match std::fs::read_to_string(results_path) {
            Ok(raw) => serde_json::from_str::<ResultsFile>(&raw)
                .unwrap_or_else(|e| panic!("invalid {results_path}: {e}"))
                .results
                .into_iter()
                .map(|r| (r.match_id.clone(), r))
                .collect(),
            Err(_) => HashMap::new(),
        };

        let mut group_index: HashMap<char, Vec<usize>> = HashMap::new();
        for (i, t) in tf.teams.iter().enumerate() {
            let g = t.group.chars().next().expect("empty group");
            group_index.entry(g).or_default().push(i);
        }
        for g in GROUPS {
            let v = group_index.get_mut(&g).unwrap_or_else(|| panic!("group {g} missing"));
            v.sort_by_key(|&i| tf.teams[i].pos);
            assert_eq!(v.len(), 4, "group {g} must have 4 teams");
        }

        Data {
            teams: tf.teams,
            hosts: tf.hosts,
            results,
            group_index,
        }
    }

    /// Kickoff times (match id -> ISO-8601 UTC) for ordering and display. Empty if file missing.
    pub fn load_schedule(path: &str) -> HashMap<String, String> {
        std::fs::read_to_string(path)
            .ok()
            .and_then(|raw| serde_json::from_str::<HashMap<String, String>>(&raw).ok())
            .unwrap_or_default()
    }

    /// Optional Opta benchmark win-% keyed by team code. Empty if file missing.
    pub fn load_opta(path: &str) -> HashMap<String, f64> {
        #[derive(Deserialize)]
        struct OptaFile {
            win_pct: HashMap<String, f64>,
        }
        std::fs::read_to_string(path)
            .ok()
            .and_then(|raw| serde_json::from_str::<OptaFile>(&raw).ok())
            .map(|f| f.win_pct)
            .unwrap_or_default()
    }

    pub fn is_host(&self, team: usize) -> bool {
        self.hosts.contains(&self.teams[team].code)
    }

    /// Team indices of a group ordered by their draw position (1..=4).
    pub fn group_teams(&self, g: char) -> &[usize] {
        &self.group_index[&g]
    }
}
