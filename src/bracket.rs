use crate::model::GROUPS;

/// Order in which the 6 matches of a 4-team group are played (home pos, away pos),
/// grouped into matchdays 1-3. FIFA standard rotation.
pub const GROUP_FIXTURES: [(usize, usize); 6] = [(1, 2), (3, 4), (1, 3), (4, 2), (4, 1), (2, 3)];

pub fn group_match_id(group: char, home_pos: usize, away_pos: usize) -> String {
    format!("{group}:{home_pos}v{away_pos}")
}

/// What feeds one side of a knockout match.
#[derive(Clone, Copy)]
pub enum Source {
    Winner(char),
    RunnerUp(char),
    /// One of the 8 best third-placed teams. Which group feeds which slot is set by the
    /// FIFA Annex C table (see `third_table`), not stored here.
    Third,
    MatchWinner(u32),
}

pub struct KoMatch {
    pub id: u32,
    pub round: Round,
    pub a: Source,
    pub b: Source,
}

#[derive(Clone, Copy, PartialEq)]
pub enum Round {
    R32,
    R16,
    Qf,
    Sf,
    Final,
}

use Source::*;

/// Full 2026 knockout bracket (matches 73-104, third-place play-off M103 omitted).
pub fn knockout() -> Vec<KoMatch> {
    macro_rules! m {
        ($id:expr, $r:expr, $a:expr, $b:expr) => {
            KoMatch {
                id: $id,
                round: $r,
                a: $a,
                b: $b,
            }
        };
    }
    vec![
        // Round of 32
        m!(73, Round::R32, RunnerUp('A'), RunnerUp('B')),
        m!(74, Round::R32, Winner('E'), Third),
        m!(75, Round::R32, Winner('F'), RunnerUp('C')),
        m!(76, Round::R32, Winner('C'), RunnerUp('F')),
        m!(77, Round::R32, Winner('I'), Third),
        m!(78, Round::R32, RunnerUp('E'), RunnerUp('I')),
        m!(79, Round::R32, Winner('A'), Third),
        m!(80, Round::R32, Winner('L'), Third),
        m!(81, Round::R32, Winner('D'), Third),
        m!(82, Round::R32, Winner('G'), Third),
        m!(83, Round::R32, RunnerUp('K'), RunnerUp('L')),
        m!(84, Round::R32, Winner('H'), RunnerUp('J')),
        m!(85, Round::R32, Winner('B'), Third),
        m!(86, Round::R32, Winner('J'), RunnerUp('H')),
        m!(87, Round::R32, Winner('K'), Third),
        m!(88, Round::R32, RunnerUp('D'), RunnerUp('G')),
        // Round of 16
        m!(89, Round::R16, MatchWinner(74), MatchWinner(77)),
        m!(90, Round::R16, MatchWinner(73), MatchWinner(75)),
        m!(91, Round::R16, MatchWinner(76), MatchWinner(78)),
        m!(92, Round::R16, MatchWinner(79), MatchWinner(80)),
        m!(93, Round::R16, MatchWinner(83), MatchWinner(84)),
        m!(94, Round::R16, MatchWinner(81), MatchWinner(82)),
        m!(95, Round::R16, MatchWinner(86), MatchWinner(88)),
        m!(96, Round::R16, MatchWinner(85), MatchWinner(87)),
        // Quarterfinals
        m!(97, Round::Qf, MatchWinner(89), MatchWinner(90)),
        m!(98, Round::Qf, MatchWinner(93), MatchWinner(94)),
        m!(99, Round::Qf, MatchWinner(91), MatchWinner(92)),
        m!(100, Round::Qf, MatchWinner(95), MatchWinner(96)),
        // Semifinals
        m!(101, Round::Sf, MatchWinner(97), MatchWinner(98)),
        m!(102, Round::Sf, MatchWinner(99), MatchWinner(100)),
        // Final
        m!(104, Round::Final, MatchWinner(101), MatchWinner(102)),
    ]
}

pub fn all_groups() -> [char; 12] {
    GROUPS
}
