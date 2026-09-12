//! `bap-store` opens the window. `bap-store <subcommand>` is the text mode:
//! the same core with a table printer, which is how the sources are exercised
//! on a machine without a display.

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if let Some(first) = args.first()
        && first != "--"
        && !first.starts_with('-')
    {
        std::process::exit(bap_store_lib::cli::run(&args));
    }
    bap_store_lib::run();
}
