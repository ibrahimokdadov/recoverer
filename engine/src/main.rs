// engine/src/main.rs
pub mod commands;
pub mod error;
pub mod events;

fn main() {
    env_logger::init();
    log::info!("Recoverer engine starting");
}

