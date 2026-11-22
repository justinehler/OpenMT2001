//! Developer-facing input inspection; this is not the game renderer.
use openmt2001::{board::Board, pe::PeImage};
use std::{env, fmt::Write, fs};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = env::args().collect();
    if args.len() != 3 {
        return Err("usage: cargo run --bin inspect -- <v1.4 mc.exe> <output.svg>".into());
    }
    let bytes = fs::read(&args[1])?;
    let board = Board::from_v14(&PeImage::parse(&bytes)?)?;
    let mut svg = String::from(
        "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"1300\" height=\"1240\" viewBox=\"-340 -370 1100 1050\"><rect x=\"-340\" y=\"-370\" width=\"1100\" height=\"1050\" fill=\"#142332\"/><g font-family=\"sans-serif\">",
    );
    for road in &board.roads {
        writeln!(
            svg,
            "<rect x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\" fill=\"{}\"/>",
            road.center[0] - road.size[0] * 0.5,
            -road.center[1] - road.size[1] * 0.5,
            road.size[0],
            road.size[1],
            if road.elevated { "#9992b2" } else { "#697580" }
        )?;
    }
    for block in &board.blocks {
        println!(
            "{:02} {:25} center={:?} size={:?} flags={} sides={}",
            block.id, block.name, block.center, block.size, block.flags, block.street_sides
        );
        writeln!(
            svg,
            "<rect x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\" fill=\"#3c5a43\" stroke=\"#223527\"/><text x=\"{}\" y=\"{}\" fill=\"#e6e6d5\" font-size=\"6\" text-anchor=\"middle\">{}<tspan x=\"{}\" dy=\"11\">{}</tspan></text>",
            block.center[0] - block.size[0] * 0.5,
            -block.center[1] - block.size[1] * 0.5,
            block.size[0],
            block.size[1],
            block.center[0],
            -block.center[1],
            block.id,
            block.center[0],
            block.name.replace('_', " ")
        )?;
    }
    svg.push_str("<text x=\"-320\" y=\"610\" fill=\"white\" font-size=\"17\">OpenMT2001 — v1.4 topology inspection</text><text x=\"-320\" y=\"631\" fill=\"#aab8c5\" font-size=\"11\">Original executable map and recovered initialization. Diagnostic colors; not game artwork.</text></g></svg>");
    fs::write(&args[2], svg)?;
    println!(
        "{} roads/intersections, {} active blocks",
        board.roads.len(),
        board.blocks.len()
    );
    Ok(())
}
