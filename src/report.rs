use crate::analysis::{Accuracy, Played};
use crate::history::History;
use crate::model::Data;
use crate::sim::{likely_score, match_odds, Config, Tally};
use std::collections::HashMap;

const PALETTE: [&str; 8] = [
    "#46a77a", "#5b8cc4", "#c08aa4", "#c2a157", "#8a83bf", "#b87070", "#5fa39a", "#9a8a6b",
];

fn pct(n: u64, total: u64) -> f64 {
    if total == 0 { 0.0 } else { 100.0 * n as f64 / total as f64 }
}

/// Heat-shaded probability cell (single accent hue, alpha ∝ probability). p in 0..100.
fn heat(p: f64) -> String {
    if p <= 0.0 {
        return "<td class=\"ht z\">·</td>".into();
    }
    let a = (p / 100.0).sqrt() * 0.6;
    let txt = if p < 1.0 { "&lt;1".to_string() } else { format!("{p:.0}") };
    format!("<td class=\"ht\" style=\"background:rgba(var(--accent-rgb),{a:.3})\">{txt}</td>")
}

fn signed(v: f64, hide_small: bool) -> String {
    if hide_small && v.abs() < 0.1 {
        return "<td class=\"z\">·</td>".into();
    }
    let cls = if v > 0.05 { "up" } else if v < -0.05 { "down" } else { "z" };
    let sign = if v >= 0.0 { "+" } else { "" };
    format!("<td class=\"{cls}\">{sign}{v:.1}</td>")
}

fn form_cell(form: f64) -> String {
    let v = form.round() as i64;
    if v == 0 {
        "<td class=\"z\">·</td>".into()
    } else if v > 0 {
        format!("<td class=\"up\">+{v}</td>")
    } else {
        format!("<td class=\"down\">{v}</td>")
    }
}

fn opta_cells(ours: f64, opta: Option<f64>) -> String {
    match opta {
        None => "<td></td><td></td>".into(),
        Some(o) => {
            let diff = ours - o;
            let cls = if diff.abs() < 1.0 { "z" } else if diff > 0.0 { "up" } else { "down" };
            let sign = if diff >= 0.0 { "+" } else { "" };
            format!("<td class=\"opta\">{o:.1}</td><td class=\"{cls}\">{sign}{diff:.1}</td>")
        }
    }
}

/// SVG line chart of title probability across the saved snapshots.
fn title_race_svg(hist: &History, code_name: &HashMap<String, String>, top: &[String]) -> String {
    let snaps = &hist.snapshots;
    if snaps.len() < 2 {
        return String::new();
    }
    let (w, h) = (920.0, 300.0);
    let (pl, pr, pt, pb) = (34.0, 86.0, 14.0, 26.0);
    let plot_w = w - pl - pr;
    let plot_h = h - pt - pb;
    let n = snaps.len();

    let ymax = top
        .iter()
        .flat_map(|c| snaps.iter().map(move |s| *s.champion.get(c).unwrap_or(&0.0)))
        .fold(1.0_f64, f64::max);
    let ymax = (ymax / 5.0).ceil() * 5.0;

    let x = |i: usize| pl + if n > 1 { i as f64 / (n - 1) as f64 * plot_w } else { 0.0 };
    let y = |v: f64| pt + (1.0 - v / ymax) * plot_h;

    let mut s = format!(
        "<svg viewBox=\"0 0 {w} {h}\" class=\"chart\" preserveAspectRatio=\"xMidYMid meet\">"
    );

    // horizontal gridlines + y labels
    let steps = (ymax / 5.0).round() as i32;
    for k in 0..=steps {
        let v = k as f64 * 5.0;
        let yy = y(v);
        s.push_str(&format!(
            "<line x1=\"{pl}\" y1=\"{yy:.1}\" x2=\"{:.1}\" y2=\"{yy:.1}\" class=\"grid\"/><text x=\"{:.1}\" y=\"{:.1}\" class=\"yl\">{v:.0}</text>",
            pl + plot_w, pl - 6.0, yy + 3.0
        ));
    }
    // x labels
    for (i, snap) in snaps.iter().enumerate() {
        s.push_str(&format!(
            "<text x=\"{:.1}\" y=\"{:.1}\" class=\"xl\">{}</text>",
            x(i), h - 8.0, snap.label
        ));
    }
    // one line per team
    for (idx, code) in top.iter().enumerate() {
        let color = PALETTE[idx % PALETTE.len()];
        let pts: String = snaps
            .iter()
            .enumerate()
            .map(|(i, snap)| format!("{:.1},{:.1}", x(i), y(*snap.champion.get(code).unwrap_or(&0.0))))
            .collect::<Vec<_>>()
            .join(" ");
        s.push_str(&format!(
            "<polyline points=\"{pts}\" fill=\"none\" stroke=\"{color}\" stroke-width=\"2\"/>"
        ));
        let last = snaps.last().unwrap();
        let lv = *last.champion.get(code).unwrap_or(&0.0);
        s.push_str(&format!(
            "<circle cx=\"{:.1}\" cy=\"{:.1}\" r=\"2.5\" fill=\"{color}\"/><text x=\"{:.1}\" y=\"{:.1}\" class=\"ll\" fill=\"{color}\">{}</text>",
            x(n - 1), y(lv), x(n - 1) + 6.0, y(lv) + 3.0,
            code_name.get(code).map(|s| s.as_str()).unwrap_or(code)
        ));
    }
    s.push_str("</svg>");
    s
}

// ---- projected knockout bracket ----

#[derive(Clone, Copy)]
enum Child {
    Match(u32),
    LeafA(u32),
    LeafB(u32),
}

fn bracket_children() -> HashMap<u32, (Child, Child)> {
    use crate::bracket::{knockout, Round, Source};
    let mut ch = HashMap::new();
    for km in knockout() {
        let entry = if km.round == Round::R32 {
            (Child::LeafA(km.id), Child::LeafB(km.id))
        } else {
            let feeder = |s: &Source| match s {
                Source::MatchWinner(n) => Child::Match(*n),
                _ => unreachable!("non-R32 knockout side must be a match winner"),
            };
            (feeder(&km.a), feeder(&km.b))
        };
        ch.insert(km.id, entry);
    }
    ch
}

fn collect_leaves(m: u32, ch: &HashMap<u32, (Child, Child)>, out: &mut Vec<(u32, bool)>) {
    let (a, b) = ch[&m];
    for c in [a, b] {
        match c {
            Child::Match(x) => collect_leaves(x, ch, out),
            Child::LeafA(x) => out.push((x, false)),
            Child::LeafB(x) => out.push((x, true)),
        }
    }
}

fn modal(counts: &[u64], sims: u64) -> Option<(usize, f64)> {
    let (i, &c) = counts.iter().enumerate().max_by_key(|(_, &c)| c)?;
    (c > 0).then_some((i, 100.0 * c as f64 / sims as f64))
}

fn round_col(id: u32) -> usize {
    match id {
        73..=88 => 1,
        89..=96 => 2,
        97..=100 => 3,
        101..=102 => 4,
        _ => 5,
    }
}

fn bracket_svg(data: &Data, cfg: &Config, t: &Tally) -> String {
    let ch = bracket_children();
    let mut order: Vec<(u32, bool)> = Vec::new();
    collect_leaves(104, &ch, &mut order);
    let leaf_index: HashMap<(u32, bool), usize> =
        order.iter().enumerate().map(|(i, &k)| (k, i)).collect();

    // parent of each feeder match
    let mut parent: HashMap<u32, u32> = HashMap::new();
    for (&p, &(a, b)) in &ch {
        for c in [a, b] {
            if let Child::Match(x) = c {
                parent.insert(x, p);
            }
        }
    }

    let center_idx = |m: u32| -> f64 {
        let mut v = Vec::new();
        collect_leaves(m, &ch, &mut v);
        let (mut lo, mut hi) = (usize::MAX, 0usize);
        for k in v {
            let i = leaf_index[&k];
            lo = lo.min(i);
            hi = hi.max(i);
        }
        (lo + hi) as f64 / 2.0
    };

    let (left, top) = (12.0, 46.0);
    let (col_w, row_h) = (150.0, 30.0);
    let (box_w, box_h) = (132.0, 24.0);
    let width = left + 6.0 * col_w + 6.0;
    let height = top + 32.0 * row_h + 12.0;
    let x = |col: usize| left + col as f64 * col_w;
    let y_leaf = |i: usize| top + i as f64 * row_h;
    let y_mid = |c: f64| top + c * row_h;

    let mut s = format!(
        "<svg viewBox=\"0 0 {width:.0} {height:.0}\" class=\"bracket\" preserveAspectRatio=\"xMinYMin meet\">"
    );

    // column headers
    for (c, label) in ["Round of 32", "Round of 16", "Quarter", "Semi", "Final", "Champion"].iter().enumerate() {
        s.push_str(&format!(
            "<text x=\"{:.1}\" y=\"26\" class=\"bh\">{label}</text>",
            x(c) + box_w / 2.0
        ));
    }

    let box_team = |team: Option<(usize, f64)>| -> (String, String) {
        match team {
            Some((idx, p)) => (data.teams[idx].code.clone(), format!("{p:.0}%")),
            None => ("—".into(), String::new()),
        }
    };

    // connectors first (so boxes draw on top)
    let elbow = |x1: f64, y1: f64, x2: f64, y2: f64| -> String {
        let mx = (x1 + x2) / 2.0;
        format!("<path d=\"M{x1:.1} {y1:.1} H{mx:.1} V{y2:.1} H{x2:.1}\" class=\"bc\"/>")
    };
    // leaf -> its R32 winner box
    for &(m, side) in &order {
        let i = leaf_index[&(m, side)];
        let y1 = y_leaf(i) + box_h / 2.0;
        let y2 = y_mid(center_idx(m)) + box_h / 2.0;
        s.push_str(&elbow(x(0) + box_w, y1, x(1), y2));
    }
    // winner box -> parent winner box
    for (&m, _) in &ch {
        if let Some(&p) = parent.get(&m) {
            let y1 = y_mid(center_idx(m)) + box_h / 2.0;
            let y2 = y_mid(center_idx(p)) + box_h / 2.0;
            s.push_str(&elbow(x(round_col(m)) + box_w, y1, x(round_col(p)), y2));
        }
    }

    let draw_box = |sx: f64, sy: f64, code: &str, pct: &str, cls: &str| -> String {
        let label = if pct.is_empty() {
            format!("<tspan class=\"bn\">{code}</tspan>")
        } else {
            format!("<tspan class=\"bn\">{code}</tspan> <tspan class=\"bp\">{pct}</tspan>")
        };
        format!(
            "<g class=\"{cls}\"><rect x=\"{sx:.1}\" y=\"{sy:.1}\" width=\"{box_w}\" height=\"{box_h}\" rx=\"4\"/><text x=\"{:.1}\" y=\"{:.1}\">{label}</text></g>",
            sx + 8.0, sy + box_h / 2.0 + 4.0
        )
    };

    // R32 participant leaves (col 0)
    for &(m, side) in &order {
        let i = leaf_index[&(m, side)];
        let counts = if side { &t.slot_b[&m] } else { &t.slot_a[&m] };
        let (code, pct) = box_team(modal(counts, t.sims));
        s.push_str(&draw_box(x(0), y_leaf(i), &code, &pct, "b32"));
    }
    // winner boxes for every match
    for km in crate::bracket::knockout() {
        let (code, pct) = box_team(modal(&t.win[&km.id], t.sims));
        let cls = if km.id == 104 { "bchamp" } else { "bwin" };
        s.push_str(&draw_box(x(round_col(km.id)), y_mid(center_idx(km.id)), &code, &pct, cls));
    }

    // most likely scoreline of each match, at its connector merge point
    for km in crate::bracket::knockout() {
        let (pa, pb) = if (73..=88).contains(&km.id) {
            (modal(&t.slot_a[&km.id], t.sims), modal(&t.slot_b[&km.id], t.sims))
        } else {
            let (ca, cb) = ch[&km.id];
            let feed = |c: Child| if let Child::Match(x) = c { modal(&t.win[&x], t.sims) } else { None };
            (feed(ca), feed(cb))
        };
        if let (Some((ai, _)), Some((bi, _))) = (pa, pb) {
            // actual result (with how it was decided) if played, else the most likely scoreline
            let (label, cls) = match data.results.get(&format!("M{}", km.id)) {
                Some(r) => {
                    let tag = match r.decided.as_deref() {
                        Some("aet") => " a.e.t.",
                        Some("pens") => " pens",
                        _ => "",
                    };
                    (format!("{}–{}{tag}", r.home, r.away), "bs played")
                }
                None => {
                    let (gh, gv) = likely_score(data, cfg, ai, bi);
                    (format!("{gh}–{gv}"), "bs")
                }
            };
            let col = round_col(km.id);
            let mx = (x(col - 1) + box_w + x(col)) / 2.0;
            let my = y_mid(center_idx(km.id)) + box_h / 2.0 + 4.0;
            s.push_str(&format!("<text x=\"{mx:.1}\" y=\"{my:.1}\" class=\"{cls}\">{label}</text>"));
        }
    }

    s.push_str("</svg>");
    s
}

/// Flag emoji for a FIFA team code (ISO-2 -> regional indicators; subdivisions hardcoded).
fn flag(code: &str) -> String {
    let iso = match code {
        "ENG" => return "🏴\u{E0067}\u{E0062}\u{E0065}\u{E006E}\u{E0067}\u{E007F}".into(),
        "SCO" => return "🏴\u{E0067}\u{E0062}\u{E0073}\u{E0063}\u{E0074}\u{E007F}".into(),
        "MEX" => "MX", "RSA" => "ZA", "KOR" => "KR", "CZE" => "CZ", "CAN" => "CA",
        "BIH" => "BA", "QAT" => "QA", "SUI" => "CH", "BRA" => "BR", "MAR" => "MA",
        "HAI" => "HT", "USA" => "US", "PAR" => "PY", "AUS" => "AU", "TUR" => "TR",
        "GER" => "DE", "CUW" => "CW", "CIV" => "CI", "ECU" => "EC", "NED" => "NL",
        "JPN" => "JP", "SWE" => "SE", "TUN" => "TN", "BEL" => "BE", "EGY" => "EG",
        "IRN" => "IR", "NZL" => "NZ", "ESP" => "ES", "CPV" => "CV", "KSA" => "SA",
        "URU" => "UY", "FRA" => "FR", "SEN" => "SN", "IRQ" => "IQ", "NOR" => "NO",
        "ARG" => "AR", "ALG" => "DZ", "AUT" => "AT", "JOR" => "JO", "POR" => "PT",
        "COD" => "CD", "UZB" => "UZ", "COL" => "CO", "CRO" => "HR", "GHA" => "GH",
        "PAN" => "PA",
        _ => return "🏳️".into(),
    };
    iso.chars()
        .map(|c| char::from_u32(0x1F1E6 + (c as u32 - 'A' as u32)).unwrap())
        .collect()
}

struct TableRow {
    team: usize,
    pld: u32,
    w: u32,
    d: u32,
    l: u32,
    gd: i32,
    pts: u32,
}

/// Current group table from the results entered so far (works with a partial group).
fn current_table(data: &Data, g: char) -> Vec<TableRow> {
    use crate::bracket::{group_match_id, GROUP_FIXTURES};
    let teams = data.group_teams(g);
    let mut row: Vec<TableRow> = teams
        .iter()
        .map(|&t| TableRow { team: t, pld: 0, w: 0, d: 0, l: 0, gd: 0, pts: 0 })
        .collect();
    let mut gf = [0i32; 4];
    let mut ga = [0i32; 4];
    for &(hp, ap) in GROUP_FIXTURES.iter() {
        if let Some(r) = data.results.get(&group_match_id(g, hp, ap)) {
            let (hi, ai) = (hp - 1, ap - 1);
            row[hi].pld += 1;
            row[ai].pld += 1;
            gf[hi] += r.home as i32;
            ga[hi] += r.away as i32;
            gf[ai] += r.away as i32;
            ga[ai] += r.home as i32;
            match r.home.cmp(&r.away) {
                std::cmp::Ordering::Greater => { row[hi].w += 1; row[hi].pts += 3; row[ai].l += 1; }
                std::cmp::Ordering::Less => { row[ai].w += 1; row[ai].pts += 3; row[hi].l += 1; }
                std::cmp::Ordering::Equal => { row[hi].d += 1; row[ai].d += 1; row[hi].pts += 1; row[ai].pts += 1; }
            }
        }
    }
    for (i, r) in row.iter_mut().enumerate() {
        r.gd = gf[i] - ga[i];
    }
    row
}

fn groups_section(data: &Data, cfg: &Config, t: &Tally) -> String {
    use crate::bracket::{all_groups, group_match_id, GROUP_FIXTURES};
    let sims = t.sims;
    let mut cards = String::new();
    for g in all_groups() {
        let mut rows = current_table(data, g);
        let qualify = |team: usize| -> f64 {
            pct(t.group_winner[team] + t.runner_up[team] + t.third_through[team], sims)
        };
        let any_played = rows.iter().any(|r| r.pld > 0);
        // order by real standings if played, otherwise by qualification probability
        rows.sort_by(|a, b| {
            if any_played {
                b.pts.cmp(&a.pts).then(b.gd.cmp(&a.gd))
            } else {
                qualify(b.team).partial_cmp(&qualify(a.team)).unwrap()
            }
        });

        let mut body = String::new();
        for (i, r) in rows.iter().enumerate() {
            let q = qualify(r.team);
            let exp_pts = if sims > 0 { t.points_sum[r.team] as f64 / sims as f64 } else { 0.0 };
            let zone = if i < 2 { "q1" } else { "qo" };
            let code = &data.teams[r.team].code;
            let pts_or_exp = if any_played {
                format!("{}", r.pts)
            } else {
                format!("{exp_pts:.1}")
            };
            body.push_str(&format!(
                "<tr class=\"{zone}\"><td class=\"gp\">{}</td><td class=\"gt\">{} {}</td><td>{}</td><td>{:+}</td><td class=\"gpts\">{}</td>{}</tr>\n",
                i + 1,
                flag(code),
                code,
                r.pld,
                r.gd,
                pts_or_exp,
                heat(q),
            ));
        }
        let pts_head = if any_played { "Pts" } else { "xPts" };

        // matchday schedule (results if played, otherwise the most likely scoreline)
        let teams = data.group_teams(g);
        let mut sched = String::new();
        for md in 0..3 {
            sched.push_str(&format!("<div class=\"md\"><span class=\"mdl\">Matchday {}</span>", md + 1));
            for &(hp, ap) in &GROUP_FIXTURES[md * 2..md * 2 + 2] {
                let (home, away) = (teams[hp - 1], teams[ap - 1]);
                let id = group_match_id(g, hp, ap);
                let (score, cls) = match data.results.get(&id) {
                    Some(r) => (format!("{}–{}", r.home, r.away), " done"),
                    None => {
                        let (gh, gv) = likely_score(data, cfg, home, away);
                        (format!("{gh}–{gv}"), "")
                    }
                };
                sched.push_str(&format!(
                    "<div class=\"mr\"><span class=\"mh\">{} {}</span><span class=\"ms{cls}\">{score}</span><span class=\"ma\">{} {}</span></div>",
                    data.teams[home].code, flag(&data.teams[home].code),
                    flag(&data.teams[away].code), data.teams[away].code,
                ));
            }
            sched.push_str("</div>");
        }

        cards.push_str(&format!(
            r#"<div class="gcard"><div class="ghead">Group {g}</div>
<table class="gtab"><thead><tr><th class="gp">#</th><th class="gt">Team</th><th>Pl</th><th>GD</th><th class="gpts">{pts_head}</th><th class="ht">Q%</th></tr></thead>
<tbody>{body}</tbody></table>
<div class="gsched">{sched}</div></div>
"#
        ));
    }
    format!(
        r#"<section>
<h2>Group Stage</h2>
<p class="sub">Live tables from entered results. “Q%” is each team's simulated chance to reach the knockout round (shaded by probability). Top two are highlighted; before kickoff the points column shows expected points (xPts).</p>
<div class="groups">{cards}</div>
</section>
"#
    )
}

fn path_section(data: &Data, t: &Tally) -> String {
    let sims = t.sims;
    let mut idx: Vec<usize> = (0..t.n).collect();
    idx.sort_by(|&a, &b| {
        t.champion[b]
            .cmp(&t.champion[a])
            .then(t.reach_final[b].cmp(&t.reach_final[a]))
    });

    // probability a team reaches (plays in) each knockout round, 0..4
    let reach_p = |i: usize, r: usize| -> f64 {
        let n = match r {
            0 => t.group_winner[i] + t.runner_up[i] + t.third_through[i],
            1 => t.reach_r16[i],
            2 => t.reach_qf[i],
            3 => t.reach_sf[i],
            _ => t.reach_final[i],
        };
        pct(n, sims)
    };

    let mut rows = String::new();
    for &i in &idx {
        if reach_p(i, 0) < 0.1 {
            continue;
        }
        let mut cells = String::new();
        for r in 0..5 {
            let rp = reach_p(i, r);
            let counts = &t.opp[i][r];
            let sum: u64 = counts.iter().sum();
            if sum == 0 {
                cells.push_str("<td class=\"z\">·</td>");
                continue;
            }
            let (oi, &c) = counts.iter().enumerate().max_by_key(|(_, &c)| c).unwrap();
            let cond = 100.0 * c as f64 / sum as f64;
            let a = (rp / 100.0).sqrt() * 0.72;
            cells.push_str(&format!(
                "<td class=\"pc\" style=\"background:rgba(var(--accent-rgb),{a:.3})\">{} {} <small>{cond:.0}%</small></td>",
                flag(&data.teams[oi].code),
                data.teams[oi].code,
            ));
        }
        rows.push_str(&format!(
            "<tr><td class=\"tm\">{} <span class=\"nm\">{}</span></td><td class=\"g\">{}</td>{cells}</tr>\n",
            flag(&data.teams[i].code),
            data.teams[i].name,
            data.teams[i].group,
        ));
    }

    format!(
        r#"<section class="analysis">
<h2>Path to the Title</h2>
<p class="sub">For each team, the most likely opponent in each knockout round (with how often that opponent shows up). Cell shading = the team's chance of reaching that round, so the row fades as the path gets harder.</p>
<div class="tw">
<table>
<thead><tr><th class="tm">Team</th><th class="g">Grp</th><th>Round of 32</th><th>Round of 16</th><th>Quarter</th><th>Semi</th><th>Final</th></tr></thead>
<tbody>
{rows}</tbody>
</table>
</div>
</section>
"#
    )
}

fn fixtures_section(data: &Data, cfg: &Config) -> String {
    use crate::bracket::{all_groups, group_match_id, GROUP_FIXTURES};
    let mut rows = String::new();
    for g in all_groups() {
        let teams = data.group_teams(g);
        for (md, &(hp, ap)) in GROUP_FIXTURES.iter().enumerate() {
            let (home, away) = (teams[hp - 1], teams[ap - 1]);
            let id = group_match_id(g, hp, ap);
            let o = match_odds(data, cfg, home, away);
            let (score, cls) = match data.results.get(&id) {
                Some(r) => (format!("{}–{} ✓", r.home, r.away), "sc done"),
                None => {
                    let (gh, gv) = likely_score(data, cfg, home, away);
                    (format!("{gh}–{gv}"), "sc")
                }
            };
            rows.push_str(&format!(
                "<tr><td class=\"st\">{g} · MD{}</td><td class=\"tm\">{}</td><td class=\"{cls}\">{score}</td><td class=\"tm\">{}</td><td>{:.0}</td><td>{:.0}</td><td>{:.0}</td></tr>\n",
                (md / 2) + 1,
                data.teams[home].name,
                data.teams[away].name,
                o.home * 100.0, o.draw * 100.0, o.away * 100.0,
            ));
        }
    }
    format!(
        r#"<section class="analysis">
<h2>Most Likely Results</h2>
<p class="sub">Most probable scoreline for every group match (mode of the Poisson model), plus home/draw/away odds. Played matches show the real result (✓).</p>
<div class="tw">
<table>
<thead><tr><th class="st">Match</th><th class="tm">Home</th><th class="sc">Score</th><th class="tm">Away</th><th>H%</th><th>D%</th><th>A%</th></tr></thead>
<tbody>
{rows}</tbody>
</table>
</div>
</section>
"#
    )
}

fn predictions_section(data: &Data, played: &[Played], acc: &Option<Accuracy>) -> String {
    let Some(acc) = acc else {
        return r#"<section class="analysis"><h2>Predictions vs Reality</h2><p class="sub empty">No matches played yet. Once results are entered, this tab scores how well the model called them — Brier score, skill %, and the biggest upsets.</p></section>"#.into();
    };
    let skill = (1.0 - acc.model_brier / acc.baseline_brier) * 100.0;
    let mut idx: Vec<usize> = (0..played.len()).collect();
    idx.sort_by(|&a, &b| played[a].prob_actual().partial_cmp(&played[b].prob_actual()).unwrap());

    let mut rows = String::new();
    for &i in &idx {
        let m = &played[i];
        let (hn, an) = (&data.teams[m.home].name, &data.teams[m.away].name);
        let pa = m.prob_actual() * 100.0;
        let flag = if pa < 20.0 {
            "<span class=\"up\">▲ upset</span>"
        } else if pa > 55.0 {
            "<span class=\"z\">expected</span>"
        } else {
            ""
        };
        rows.push_str(&format!(
            "<tr><td class=\"st\">{}</td><td class=\"tm\">{} <span class=\"cd\">v</span> {}</td><td class=\"sc\">{}–{}</td><td>{:.0}</td><td>{:.0}</td><td>{:.0}</td><td class=\"pa\">{:.0}%</td><td>{}</td></tr>\n",
            m.stage, hn, an,
            m.gh, m.ga,
            m.odds.home * 100.0, m.odds.draw * 100.0, m.odds.away * 100.0,
            pa, flag
        ));
    }

    format!(
        r#"<section class="analysis">
<h2>Predictions vs Reality</h2>
<p class="sub">Model called <b>{correct}/{played}</b> matches right · Brier <b>{brier:.3}</b> vs {base:.3} baseline · skill <b>{skill:.0}%</b> (higher = better). Sorted by surprise (model's probability of what actually happened).</p>
<div class="tw">
<table>
<thead><tr><th class="st">Stage</th><th class="tm">Match</th><th class="sc">Score</th><th>Home%</th><th>Draw%</th><th>Away%</th><th class="pa">P(actual)</th><th></th></tr></thead>
<tbody>
{rows}</tbody>
</table>
</div>
</section>
"#,
        correct = acc.correct_pick,
        played = acc.played,
        brier = acc.model_brier,
        base = acc.baseline_brier,
    )
}

pub fn build_html(
    data: &Data,
    cfg: &Config,
    t: &Tally,
    opta: &HashMap<String, f64>,
    hist: &History,
    played: &[Played],
    acc: &Option<Accuracy>,
) -> String {
    let total = t.sims;
    let has_opta = !opta.is_empty();

    // previous snapshot (the stage before the current one) for the delta column
    let prev_champ: Option<&HashMap<String, f64>> = if hist.snapshots.len() >= 2 {
        Some(&hist.snapshots[hist.snapshots.len() - 2].champion)
    } else {
        None
    };

    let code_name: HashMap<String, String> = data
        .teams
        .iter()
        .map(|t| (t.code.clone(), t.name.clone()))
        .collect();

    struct Row {
        i: usize,
        champ: f64,
    }
    let mut order: Vec<Row> = (0..t.n)
        .map(|i| Row { i, champ: pct(t.champion[i], total) })
        .collect();
    order.sort_by(|a, b| b.champ.partial_cmp(&a.champ).unwrap());
    let top_codes: Vec<String> = order.iter().take(6).map(|r| data.teams[r.i].code.clone()).collect();

    let mut body = String::new();
    for (rank, r) in order.iter().enumerate() {
        let i = r.i;
        let team = &data.teams[i];
        let champ = r.champ;
        let delta = match prev_champ {
            Some(p) => signed(champ - *p.get(&team.code).unwrap_or(&champ), true),
            None => "<td class=\"z\">·</td>".into(),
        };
        let opta_block = if has_opta { opta_cells(champ, opta.get(&team.code).copied()) } else { String::new() };
        body.push_str(&format!(
            "<tr><td class=\"rk\">{}</td><td class=\"tm\">{} <span class=\"nm\">{}</span> <span class=\"cd\">{}</span></td><td class=\"g\">{}</td><td class=\"elo\">{}</td>{}{}{}{}{}{}{}{}</tr>\n",
            rank + 1, flag(&team.code), team.name, team.code, team.group, team.elo as i64,
            form_cell(team.form),
            heat(champ),
            delta,
            opta_block,
            heat(pct(t.reach_final[i], total)),
            heat(pct(t.reach_sf[i], total)),
            heat(pct(t.reach_qf[i], total)),
            heat(pct(t.group_winner[i] + t.runner_up[i] + t.third_through[i], total)),
        ));
    }

    let opta_head = if has_opta { "<th class=\"opta\">Opta</th><th>vs</th>" } else { "" };
    let chart = title_race_svg(hist, &code_name, &top_codes);
    let chart_block = if chart.is_empty() {
        String::new()
    } else {
        format!("<section class=\"chart-wrap\"><h2>Title Race</h2><p class=\"sub\">Championship probability of the current top 6 across saved stages.</p>{chart}</section>")
    };
    let predictions = predictions_section(data, played, acc);
    let fixtures = fixtures_section(data, cfg);
    let groups = groups_section(data, cfg, t);
    let path = path_section(data, t);
    let bracket = bracket_svg(data, cfg, t);
    let played_n = data.results.len();
    let stages = hist.snapshots.len();
    // phase-aware default tab: Groups during the group stage, Bracket once the knockouts are set
    let group_played = data.results.keys().filter(|k| k.contains(':')).count();
    let ko_started = group_played >= 72 || data.results.keys().any(|k| k.starts_with('M'));
    let ck_groups = if ko_started { "" } else { " checked" };
    let ck_bracket = if ko_started { " checked" } else { "" };

    format!(
        r#"<!DOCTYPE html>
<html lang="en">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1">
<title>World Cup 2026 — Monte Carlo Simulation</title>
<link rel="stylesheet" href="style.css">
</head>
<body>
<header>
  <h1>World Cup 2026</h1>
  <p class="sub">{total} Monte Carlo simulations · Elo + form + Poisson · {played_n} real results in · {stages} saved stage(s)</p>
</header>
<div class="tabs">
<input type="radio" name="tab" id="tab-groups"{ck_groups}>
<input type="radio" name="tab" id="tab-bracket"{ck_bracket}>
<input type="radio" name="tab" id="tab-odds">
<input type="radio" name="tab" id="tab-path">
<input type="radio" name="tab" id="tab-fixtures">
<input type="radio" name="tab" id="tab-accuracy">
<nav class="tabbar">
  <label for="tab-groups">Groups</label>
  <label for="tab-bracket">Bracket</label>
  <label for="tab-odds">Odds</label>
  <label for="tab-path">Path</label>
  <label for="tab-fixtures">Fixtures</label>
  <label for="tab-accuracy">Accuracy</label>
</nav>
<main>
<div class="panel" id="p-bracket">
<section class="bracket-wrap">
<h2>Projected Bracket</h2>
<p class="sub">Most likely team in each slot with its probability of reaching it, plus the most likely scoreline of each tie. Fixed once real results are in; the rest follows the simulation. Scroll sideways on mobile.</p>
<div class="bracket-scroll">{bracket}</div>
</section>
{chart_block}
</div>
<div class="panel" id="p-groups">
{groups}
</div>
<div class="panel" id="p-odds">
<section>
<h2>Title Odds</h2>
<div class="tw">
<table>
<thead>
<tr>
  <th>#</th><th class="tm">Team</th><th class="g">Grp</th><th class="elo">Elo</th><th class="form">Form</th>
  <th>Title</th><th>Δ</th>{opta_head}<th>Final</th><th>Semi</th><th>QF</th><th>KO</th>
</tr>
</thead>
<tbody>
{body}</tbody>
</table>
</div>
<p class="note">All values in %. “Form” = Elo bonus/penalty from recent internationals. “Δ” = change since the previous saved stage.
“KO” = reaches the knockout stage (group top-2 or best third). “Opta/vs” = Opta supercomputer title odds and our gap to them.</p>
</section>
</div>
<div class="panel" id="p-path">
{path}
</div>
<div class="panel" id="p-fixtures">
{fixtures}
</div>
<div class="panel" id="p-accuracy">
{predictions}
</div>
</main>
</div>
</body>
</html>
"#
    )
}

pub fn print_summary(data: &Data, t: &Tally) {
    let total = t.sims;
    let mut idx: Vec<usize> = (0..t.n).collect();
    idx.sort_by(|&a, &b| t.champion[b].cmp(&t.champion[a]));
    println!("\nTop 10 title favourites ({total} sims):");
    for (rank, &i) in idx.iter().take(10).enumerate() {
        println!(
            "{:>2}. {:<22} {:>5.1}%  (final {:>4.1}%)",
            rank + 1,
            data.teams[i].name,
            pct(t.champion[i], total),
            pct(t.reach_final[i], total),
        );
    }
}
