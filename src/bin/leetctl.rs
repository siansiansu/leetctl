use leetctl::cli;
use std::process::ExitCode;
use tokio::runtime::Builder;

fn main() -> ExitCode {
    if let Err(err) = Builder::new_multi_thread()
        .enable_all()
        .build()
        .expect("Build tokio runtime failed")
        .block_on(cli::main())
    {
        // Display, not Debug: every variant of `Error` carries a written-for-humans message, and
        // `{:?}` prints the variant name and a quoted payload instead.
        eprintln!("{err}");
        return ExitCode::FAILURE;
    }
    ExitCode::SUCCESS
}
