// engine/src/main.rs
pub mod commands;
pub mod error;
pub mod events;
pub mod filetype;
pub mod scan;
pub mod store;

fn main() {
    env_logger::init();
    log::info!("Recoverer engine starting");
}

