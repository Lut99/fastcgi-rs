//  PHP.rs
//    by Lut99
//
//  Description:
//!   Example showing the usage of the FastCGI library when connecting to a running `php-fpm`
//!   instance.
//

use std::path::Path;
use std::process::ExitCode;

use clap::Parser;
use error_trace::toplevel;
use fastcgi::FastCGI;
use fastcgi::spec::{PARAM_MAX_CONNS, PARAM_MAX_REQS, PARAM_MPXS_CONNS};
use humanlog::HumanLogger;
use log::{error, info};


/***** ARGUMENTS *****/
/// Defines the arguments for this example
#[derive(Parser)]
struct Arguments {
    /// If given, shows all debug information.
    #[clap(long)]
    trace:   bool,
    /// The address of the FastCGI server.
    ///
    /// Can give as a `<hostname>:<port>`-pair.
    #[clap(short, long, default_value = "localhost:9000")]
    address: String,
}





/***** ENTRYPOINT *****/
fn main() -> ExitCode {
    // Parse args & setup logger
    let args = Arguments::parse();
    if let Err(err) = HumanLogger::terminal(if args.trace { humanlog::DebugMode::Full } else { humanlog::DebugMode::Debug }).init() {
        eprintln!("WARNING: Failed to setup logger: {err}");
    }
    info!("{} - {} v{}", env!("CARGO_BIN_NAME"), env!("CARGO_PKG_NAME"), env!("CARGO_PKG_VERSION"));

    // See if it's a socket...
    let mut fastcgi = if Path::new(&args.address).exists() {
        match FastCGI::connect_unix(&args.address) {
            Ok(res) => res,
            Err(err) => {
                error!("{}", toplevel!(("Failed to establish FCGI connection"), err));
                return ExitCode::FAILURE;
            },
        }
    } else {
        // Establish a connection
        match FastCGI::connect_addr(&args.address) {
            Ok(res) => res,
            Err(err) => {
                error!("{}", toplevel!(("Failed to establish FCGI connection"), err));
                return ExitCode::FAILURE;
            },
        }
    };

    // Request the standard parameters
    if let Err(err) = fastcgi.get_values([PARAM_MAX_CONNS, PARAM_MAX_REQS, PARAM_MPXS_CONNS]) {
        error!("{}", toplevel!(("Failed to get values from FCGI connection"), err));
        return ExitCode::FAILURE;
    }

    ExitCode::SUCCESS
}
