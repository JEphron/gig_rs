//! WIP: An over-the-top gitignore.io command-line interface
//! With typeahead search and other goodies
extern crate reqwest;
extern crate regex;
extern crate serde;
extern crate serde_json;
#[macro_use]
extern crate serde_derive;
#[macro_use]
extern crate clap;

use std::io::{self, Read, Write};
use std::str::FromStr;
use std::string::ParseError;
use clap::{ArgMatches, AppSettings};
use serde_json::Value;
use serde::Deserialize;
use reqwest::Url;
use std::collections::HashMap;
use regex::Regex;

fn main_n() {
    let args = parse_args();

    // todo: be smarter about how we load these. It shouldn't need to wait for the request every time we launch it.
    let possible_presets = get_presets().expect("couldn't load the presets");

    match args.subcommand() {
        ("edit", Some(sub_m)) => {}
        ("get", Some(sub_m)) => {
            let requested_presets: Option<Vec<String>> = sub_m.args
                .get("presets")
                .map(|matched_arg| {
                    matched_arg.vals
                        .iter()
                        .filter_map(|os_string|
                            os_string.clone().into_string().ok())
                        .collect()
                });
            do_get(requested_presets, possible_presets);
        }
        _ => {}
    }
}

fn main(){
    let url = build_url(vec![String::from("intellij")]).unwrap();
    println!("url: {}", url);
    let gitignore = fetch_gitignore(url).unwrap();
    println!("gitiginore: {:?}", gitignore);
}

fn parse_args<'a>() -> ArgMatches<'a> {
    clap_app!(gig =>
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
    ).get_matches()
}

#[derive(Deserialize)]
struct PresetData {
    id: String,
    text: String
}

fn get_presets() -> Result<Vec<PresetData>, reqwest::Error> {
    let url = "https://www.gitignore.io/dropdown/templates.json";
    let mut resp = reqwest::get(url)?;
    let presets: Vec<PresetData> = resp.json()?;
    Ok(presets)
}

fn do_get(maybe_requested_presets: Option<Vec<String>>, possible_presets: Vec<PresetData>) {
    // if the user passed in some presets, just get those.
    // otherwise show the interactive preset picker
    match maybe_requested_presets {
        Some(requested_presets) => {}
        None => {}
    }
}

fn build_url(requested_preset_ids: Vec<String>) -> Result<Url, reqwest::UrlError> {
    let base_url = "https://www.gitignore.io/api/";
    let ids = requested_preset_ids.join(",");
    Url::from_str(&format!("{}{}", base_url, ids))
}

#[derive(Debug)]
struct Gitignore {
    sections: HashMap<String, Section>
}

#[derive(Debug)]
struct Section {
    subsections: Vec<String>
}

fn fetch_gitignore(url: Url) -> Result<Gitignore, reqwest::Error> {
    let mut resp = reqwest::get(url)?;
    let mut contents = String::new();
    resp.read_to_string(&mut contents);
    let mut sections: HashMap<String, Section> = HashMap::new();
    let re = Regex::new("### .* ###").unwrap();
    let mut active_key: Option<&str> = None;

    for line in contents.lines() {
        if re.is_match(line) {
            active_key = Some(line.trim_matches('#').trim());
            sections.insert(active_key.unwrap().to_string(), Section { subsections: vec![] });
        } else {
            if let Some(key) = active_key {
                let section = sections.entry(key.to_string()).or_insert(Section { subsections: vec![] });
                section.subsections.push(line.to_string());
            }
        }
    }
    let mut gitignore = Gitignore { sections: sections };

    Ok(gitignore)
}

fn do_edit() { unimplemented!() }