//! Searches a position for a fixed time and prints each iteration.
//!
//! `cargo run --release -p dame --example think -- [FEN] [SECONDS]`

use std::time::Duration;

use dame::{Game, Limits, START_FEN, Searcher};

#[allow(clippy::print_stdout, clippy::cast_precision_loss)]
fn main() {
    let mut args = std::env::args().skip(1);
    let fen = args.next().unwrap_or_else(|| START_FEN.to_owned());
    let seconds: f64 = args.next().map_or(1.0, |s| s.parse().expect("seconds"));
    let game = Game::from_fen(&fen).expect("valid FEN");

    let mut searcher = Searcher::default();
    let limits = Limits::move_time(Duration::from_secs_f64(seconds));
    let result = searcher.search(&game, &limits, |info| {
        let pv: Vec<String> = info.pv.iter().map(ToString::to_string).collect();
        println!(
            "depth {:>2} seldepth {:>2} score {:>6} nodes {:>10} time {:>6.3}s pv {}",
            info.depth,
            info.seldepth,
            info.score,
            info.nodes,
            info.elapsed.as_secs_f64(),
            pv.join(" ")
        );
    });
    let nps = result.nodes as f64 / result.elapsed.as_secs_f64().max(1e-9);
    println!(
        "best {:?}  ({:.2} Mnps)",
        result.best_move.map(|m| m.to_string()),
        nps / 1e6
    );
}
