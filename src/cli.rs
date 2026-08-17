//! Clap Commanders
use crate::{
    cmd::{
        CompletionsArgs, DataArgs, EditArgs, ExecArgs, ListArgs, PickArgs, SetsArgs, StatArgs,
        TestArgs, TuiArgs,
    },
    err::Error,
};
use clap::{CommandFactory, Parser, Subcommand};
use env_logger::Env;
use log::LevelFilter;

/// This should be called before calling any cli method or printing any output.
pub fn reset_signal_pipe_handler() {
    #[cfg(target_family = "unix")]
    {
        use nix::sys::signal;

        unsafe {
            let _ = signal::signal(signal::Signal::SIGPIPE, signal::SigHandler::SigDfl)
                .map_err(|e| println!("{:?}", e));
        }
    }
}

/// May the Code be with You
#[derive(Parser)]
#[command(name = "leetctl", version, about = "May the Code be with You 👻")]
#[command(arg_required_else_help = true)]
pub struct Cli {
    /// Debug mode
    #[arg(short, long)]
    pub debug: bool,

    #[command(subcommand)]
    pub command: Option<Commands>,
}

#[derive(Subcommand)]
pub enum Commands {
    /// Manage Cache
    #[command(visible_alias = "d", display_order = 1)]
    Data(DataArgs),

    /// Edit question
    #[command(visible_alias = "e", display_order = 2)]
    Edit(EditArgs),

    /// Submit solution
    #[command(visible_alias = "x", display_order = 3)]
    Exec(ExecArgs),

    /// List problems
    #[command(visible_alias = "l", display_order = 4)]
    List(ListArgs),

    /// Pick a random problem, or select one by id, name, or daily challenge
    #[command(visible_alias = "p", display_order = 5)]
    Pick(PickArgs),

    /// Show the curated problem sets available to --set
    #[command(display_order = 6)]
    Sets(SetsArgs),

    /// Show simple chart about submissions
    #[command(visible_alias = "s", display_order = 7)]
    Stat(StatArgs),

    /// Test a question
    #[command(visible_alias = "t", display_order = 8)]
    Test(TestArgs),

    /// Browse problems in a terminal UI
    #[command(display_order = 9)]
    Tui(TuiArgs),

    /// Generate shell Completions
    #[command(visible_alias = "c", display_order = 10)]
    Completions(CompletionsArgs),
}

/// Get matches
pub async fn main() -> Result<(), Error> {
    reset_signal_pipe_handler();

    let cli = Cli::parse();

    if cli.debug {
        env_logger::Builder::from_env(Env::default().default_filter_or("debug")).init();
    } else {
        env_logger::Builder::new()
            .filter_level(match cli.command {
                // The TUI owns the screen; an `info!` from a cache download would paint over it.
                Some(Commands::Tui(_)) => LevelFilter::Off,
                _ => LevelFilter::Info,
            })
            .format_timestamp(None)
            .init();
    }

    match cli.command {
        Some(Commands::Data(args)) => args.run().await,
        Some(Commands::Edit(args)) => args.run().await,
        Some(Commands::Exec(args)) => args.run().await,
        Some(Commands::List(args)) => args.run().await,
        Some(Commands::Pick(args)) => args.run().await,
        Some(Commands::Sets(args)) => args.run(),
        Some(Commands::Stat(args)) => args.run().await,
        Some(Commands::Test(args)) => args.run().await,
        Some(Commands::Tui(args)) => args.run().await,
        Some(Commands::Completions(args)) => {
            let mut cmd = Cli::command();
            args.run(&mut cmd)
        }
        None => Err(Error::MatchError),
    }
}
