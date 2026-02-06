mod cli;
mod commands;
mod mcp;
mod output;
mod scanner;

use clap::Parser;

use cli::{Cli, Commands};
use mcp::server::run_mcp_server;
use output::{print_duplicates, print_organize, print_search, print_stats};

fn main() {
    let cli = Cli::parse();

    if cli.mcp {
        run_mcp_server();
        return;
    }

    match cli.command {
        Some(Commands::Stats {
            directory,
            top,
            recursive,
        }) => {
            let result = commands::stats::run_stats(&directory, top, recursive);
            print_stats(&result);
        }

        Some(Commands::Duplicates {
            directory,
            min_size,
            recursive,
        }) => {
            let result = commands::duplicates::run_duplicates(&directory, min_size, recursive);
            print_duplicates(&result);
        }

        Some(Commands::Search {
            directory,
            name,
            content,
            min_size,
            max_size,
            newer,
            older,
            recursive,
        }) => {
            let result = commands::search::run_search(&commands::search::SearchParams {
                directory: &directory,
                name_pattern: name.as_deref(),
                content_query: content.as_deref(),
                min_size: min_size.as_deref(),
                max_size: max_size.as_deref(),
                newer: newer.as_deref(),
                older: older.as_deref(),
                recursive,
            });
            print_search(&result);
        }

        Some(Commands::Organize {
            directory,
            by,
            dry_run,
            mode,
            recursive,
            output,
        }) => {
            let result = commands::organize::run_organize(
                &directory,
                &by,
                dry_run,
                &mode,
                recursive,
                output.as_deref(),
            );
            print_organize(&result);
        }

        None => {
            eprintln!("No command specified. Use --help for usage information.");
            std::process::exit(1);
        }
    }
}
