mod analysis;
mod bracket;
mod history;
mod model;
mod report;
mod sim;

use model::Data;
use rand::SeedableRng;
use sim::{simulate_tournament, Config, Tally};
use std::path::Path;

fn arg_value(flag: &str, default: &str) -> String {
    let args: Vec<String> = std::env::args().collect();
    args.iter()
        .position(|a| a == flag)
        .and_then(|i| args.get(i + 1))
        .cloned()
        .unwrap_or_else(|| default.to_string())
}

fn arg_f64(flag: &str, default: f64) -> f64 {
    let s = arg_value(flag, "");
    if s.is_empty() {
        default
    } else {
        s.parse().unwrap_or_else(|_| panic!("invalid {flag}"))
    }
}

fn has_flag(flag: &str) -> bool {
    std::env::args().any(|a| a == flag)
}

fn main() {
    let sims: u64 = arg_value("--sims", "200000").parse().expect("invalid --sims");
    let teams_path = arg_value("--teams", "data/teams.json");
    let results_path = arg_value("--results", "data/results.json");
    let out_path = arg_value("--out", "web/index.html");
    let history_path = arg_value("--history", "data/history.json");

    let d = Config::default();
    let cfg = Config {
        home_adv: arg_f64("--home-adv", d.home_adv),
        total_goals: arg_f64("--total-goals", d.total_goals),
        supremacy: arg_f64("--supremacy", d.supremacy),
        form_weight: arg_f64("--form-weight", d.form_weight),
        dc_rho: arg_f64("--dc-rho", d.dc_rho),
        dyn_k: arg_f64("--dyn-k", d.dyn_k),
    };

    let data = Data::load(&teams_path, &results_path);

    // Emit the currently-determined fixtures as JSON (used by the data-refresh script), then exit.
    if has_flag("--dump-schedule") {
        let mut items = Vec::new();
        for (id, h, a) in analysis::determined_matches(&data) {
            items.push(format!(
                "{{\"id\":\"{id}\",\"home\":\"{}\",\"away\":\"{}\"}}",
                data.teams[h].code, data.teams[a].code
            ));
        }
        println!("[{}]", items.join(","));
        return;
    }

    let mut tally = Tally::new(data.teams.len());
    let mut rng = rand::rngs::StdRng::seed_from_u64(20260611);

    for _ in 0..sims {
        simulate_tournament(&data, &cfg, &mut tally, &mut rng);
    }

    report::print_summary(&data, &tally);

    // Snapshot the current state into the history timeline (unless suppressed).
    let mut hist = history::History::load(&history_path);
    if !has_flag("--no-snapshot") {
        hist.upsert(history::snapshot(&data, &tally));
        hist.save(&history_path);
    }

    let opta = Data::load_opta(&arg_value("--opta", "data/opta.json"));
    let schedule = Data::load_schedule(&arg_value("--schedule", "data/schedule.json"));
    let played = analysis::played_matches(&data, &cfg);
    let acc = analysis::accuracy(&played);

    let html = report::build_html(&data, &cfg, &tally, &opta, &schedule, &hist, &played, &acc);
    if let Some(parent) = Path::new(&out_path).parent() {
        std::fs::create_dir_all(parent).ok();
    }
    std::fs::write(&out_path, html).expect("cannot write report");
    println!("\nReport: {out_path}");
}
