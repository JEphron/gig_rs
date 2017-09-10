//! WIP: An over-the-top gitignore.io command-line interface
//! With typeahead search and other goodies
extern crate futures;
extern crate hyper;
extern crate tokio_core;
#[macro_use]
extern crate clap;

use std::io::{self, Write};
use futures::{Future, Stream};
use hyper::Client;
use tokio_core::reactor::Core;
use clap::{ArgMatches, AppSettings};


fn main() {
    let args: ArgMatches = clap_app!(rig =>
        (setting: AppSettings::SubcommandRequiredElseHelp)
        (version: crate_version!())
        (author: crate_authors!("\n"))
        (about: "CLI interface to gitignore.io")
        (@subcommand get =>
            (about: "creates a new .gitignore for a given set of preset keywords")
            (@arg presets: +multiple "User arguments")
        )
        (@subcommand edit =>
            (about: "edit which presets are included in a .gitignore")
        )
    ).get_matches();

    match args.subcommand() {
        ("edit", Some(sub_m)) => println!("{:?}", sub_m),
        ("get", Some(sub_m)) => do_get(sub_m.args
            .get("presets")
            .map(|matched_arg| {
                matched_arg.vals
                    .iter()
                    .filter_map(|os_string|
                        os_string.clone().into_string().ok())
                    .collect()
            })
        ),
        _ => {}
    }

    // fetch the templates.json file to use as the typeahead
    //
    // https://www.gitignore.io/dropdown/templates.json
}


fn do_get(maybe_presets: Option<Vec<String>>) { println!("{:?}", maybe_presets); }

fn do_edit() { unimplemented!() }