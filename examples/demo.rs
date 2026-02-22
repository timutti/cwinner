//! Demo of all three celebration types.
//! Run: cargo run --example demo

use cwinner_lib::celebration::CelebrationLevel;
use cwinner_lib::renderer::render;
use cwinner_lib::state::State;
use std::io::{self, Write};

fn main() {
    let mut state = State::default();
    state.xp = 1325;
    state.level = 3;
    state.level_name = "Vibe Architect".into();

    println!("cwinner celebration demo");
    println!("========================");
    println!();
    println!("1) Mini   — progress bar (3s)");
    println!("2) Medium — toast (1.5s)");
    println!("3) Medium — achievement toast (2.5s)");
    println!("4) Epic   — confetti + splash (6.5s)");
    println!("q) Quit");
    println!();

    let tty = "/dev/tty".to_string();

    loop {
        print!("Choose [1-4/q]: ");
        io::stdout().flush().unwrap();

        let mut input = String::new();
        io::stdin().read_line(&mut input).unwrap();

        match input.trim() {
            "1" => {
                println!("  -> Mini celebration...");
                render(&tty, &CelebrationLevel::Mini, &state, None);
                println!("  Done!");
            }
            "2" => {
                println!("  -> Medium toast...");
                render(&tty, &CelebrationLevel::Medium, &state, None);
                println!("  Done!");
            }
            "3" => {
                println!("  -> Achievement toast...");
                render(
                    &tty,
                    &CelebrationLevel::Medium,
                    &state,
                    Some("First Commit"),
                );
                println!("  Done!");
            }
            "4" => {
                println!("  -> Epic celebration...");
                render(
                    &tty,
                    &CelebrationLevel::Epic,
                    &state,
                    Some("ACHIEVEMENT UNLOCKED!"),
                );
                println!("  Done!");
            }
            "q" | "Q" => {
                println!("Bye!");
                break;
            }
            _ => println!("  Invalid choice, try 1-4 or q"),
        }
        println!();
    }
}
