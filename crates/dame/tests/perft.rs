//! Perft node counts against published reference numbers.
//!
//! All counts treat capture sequences with the same origin, destination and
//! captured set as one move. Sources: the perft tables of Rein Halbersma's
//! dctl (`test/src/algo/traversal`) and the damforum.nl perft thread
//! (<https://damforum.nl/bb3/viewtopic.php?f=53&t=2308>), cross-checked
//! with an independent reference implementation.
//!
//! The shallow depths run in every build; deeper ones are cheap in release
//! builds (`cargo test --release`) and are skipped in debug builds.

use dame::{Position, perft};

/// The deepest depth to test: everything in release, a quick subset in debug.
const fn max_nodes() -> u64 {
    if cfg!(debug_assertions) {
        2_000_000
    } else {
        u64::MAX
    }
}

fn check(fen: &str, expected: &[u64]) {
    let pos = Position::from_fen(fen).unwrap();
    for (depth, &nodes) in (1..).zip(expected) {
        if nodes > max_nodes() {
            break;
        }
        assert_eq!(perft(&pos, depth), nodes, "{fen} at depth {depth}");
    }
}

#[test]
fn initial_position() {
    check(
        "W:W31-50:B1-20",
        &[
            9, 81, 658, 4_265, 27_117, 167_140, 1_049_442, 6_483_961, 41_022_423,
        ],
    );
}

#[test]
fn woldouby() {
    check(
        "W:B12,13,14,16,18,19,21,23,24,26:W25,27,28,30,32,33,34,35,37,38",
        &[
            6, 12, 30, 73, 215, 590, 1_944, 6_269, 22_369, 88_050, 377_436, 1_910_989, 9_872_645,
        ],
    );
}

#[test]
fn random_position_with_kings() {
    check(
        "B:BK17,K24:W6,9,10,11,20,21,22,23,30,K31,33,37,41,42,43,44,46",
        &[14, 55, 1_168, 5_432, 87_195, 629_010, 9_041_010],
    );
}

#[test]
fn all_kings() {
    check(
        "W:WK31-50:BK1-20",
        &[
            17, 79, 352, 1_399, 7_062, 37_589, 217_575, 1_333_217, 8_558_321,
        ],
    );
}

#[test]
fn almost_promotion() {
    check(
        "W:W6,7,8,9,10:B41,42,43,44,45",
        &[9, 81, 795, 7_578, 86_351, 936_311, 11_448_262],
    );
}
