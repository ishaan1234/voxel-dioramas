use dioramas::{engine, scenes};

fn main() {
    match engine::app::parse_args(scenes::REGISTRY) {
        Ok(opts) => engine::app::run(scenes::REGISTRY, opts),
        Err(e) => {
            eprintln!("{e}");
            std::process::exit(2);
        }
    }
}
