use clap::{Arg, ArgAction, ArgMatches, Command};

pub fn get_subcommand() -> Command {
    Command::new("render")
        .about("Render an html report from one or more JSON result files")
        .args(&[Arg::new("json_files")
            .required(true)
            .num_args(1..)
            .trailing_var_arg(true)
            .help("Specifies one or more JSON files")])
}
