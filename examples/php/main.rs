//  PHP.rs
//    by Lut99
//
//  Description:
//!   Example showing the usage of the FastCGI library when connecting to a running `php-fpm`
//!   instance.
//

use std::collections::HashMap;
use std::net::{SocketAddr, ToSocketAddrs};
use std::process::ExitCode;

use clap::{Parser, Subcommand};
use error_trace::toplevel;
use fastcgi::fcgi::FastCGI;
use fastcgi::spec::{PARAM_MAX_CONNS, PARAM_MAX_REQS, PARAM_MPXS_CONNS};
use humanlog::HumanLogger;
use log::{debug, error, info};
use tokio::net::TcpStream;


/***** ARGUMENTS *****/
/// Defines the arguments for this example
#[derive(Parser)]
struct Arguments {
    /// If given, shows all debug information.
    #[clap(long, global = true)]
    trace:   bool,
    /// The address of the FastCGI server.
    ///
    /// Can give as a `<hostname>:<port>`-pair.
    #[clap(short, long, default_value = "localhost:9000")]
    address: String,

    /// A subcommand to execute
    #[clap(subcommand)]
    cmd: Command,
}

#[derive(Clone, Subcommand)]
enum Command {
    #[clap(name = "params", about = "Read the value of some parameters from the PHP application.")]
    Params {
        #[clap(short, long, help = "If given, reads these parameters from the application instead of the default three.")]
        params: Option<Vec<String>>,
    },
}





/***** ENTRYPOINT *****/
#[tokio::main(flavor = "current_thread")]
async fn main() -> ExitCode {
    // Parse args & setup logger
    let args = Arguments::parse();
    if let Err(err) = HumanLogger::terminal(if args.trace { humanlog::DebugMode::Full } else { humanlog::DebugMode::Debug }).init() {
        eprintln!("WARNING: Failed to setup logger: {err}");
    }
    info!("{} - {} v{}", env!("CARGO_BIN_NAME"), env!("CARGO_PKG_NAME"), env!("CARGO_PKG_VERSION"));

    // Resolve the address
    let addr: SocketAddr = match args.address.to_socket_addrs() {
        Ok(mut addr) => match addr.next() {
            Some(addr) => addr,
            None => {
                error!("Address {:?} did not resolve to any IP-addresses", args.address);
                return ExitCode::FAILURE;
            },
        },
        Err(err) => {
            error!("{}", toplevel!(("Failed to resolve address {:?}", args.address), err));
            return ExitCode::FAILURE;
        },
    };
    debug!("Resolved {:?} to {addr:?}", args.address);

    // Establish a connection
    let fastcgi = match TcpStream::connect(addr).await {
        Ok(res) => FastCGI::new(res),
        Err(err) => {
            error!("{}", toplevel!(("Failed to establish TCP connection to {addr:?}"), err));
            return ExitCode::FAILURE;
        },
    };
    debug!("Connected to {addr:?}");

    match args.cmd {
        Command::Params { params } => {
            // Request the standard parameters
            // Looks intimidating, mostly just creates iterators for parameters
            let values: HashMap<String, String> = match fastcgi
                .get_values(if let Some(params) = &params {
                    Box::new(params.iter().map(String::as_str)) as Box<dyn Iterator<Item = &str>>
                } else {
                    Box::new([PARAM_MAX_CONNS, PARAM_MAX_REQS, PARAM_MPXS_CONNS].into_iter())
                })
                .await
            {
                Ok(res) => res,
                Err(err) => {
                    error!("{}", toplevel!(("Failed to get values from FCGI connection"), err));
                    return ExitCode::FAILURE;
                },
            };
            // Print them
            println!("Application parameters:");
            for (name, value) in values.into_iter() {
                println!("  {name:?}: {value:?}");
            }
            ExitCode::SUCCESS
        },
    }
}
