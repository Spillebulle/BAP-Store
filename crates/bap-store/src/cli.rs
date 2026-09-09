//! Text mode. TODO: search, sources, updates, installed.

pub fn run(args: &[String]) -> i32 {
    eprintln!("bap-store: unknown subcommand {:?}", args.first());
    2
}
