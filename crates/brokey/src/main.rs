//! `brokey` opens the window. `brokey <subcommand>` is the text mode:
//! the same core with a table printer, which is how the sources are exercised
//! on a machine without a display.

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if let Some(first) = args.first()
        && first != "--"
        && !first.starts_with('-')
    {
        std::process::exit(brokey_lib::cli::run(&args));
    }
    brokey_lib::run();
}
