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
#[macro_use]
extern crate itertools;

use clap::{ArgMatches, AppSettings};
use reqwest::Url;
use regex::Regex;
use std::io::{self, Read, Write};
use std::str::FromStr;
use std::string::ParseError;
use std::error::Error;
use std::fs::File;
use std::path::{PathBuf, Path};
use std::collections::HashMap;
use serde_json::Value;
use serde::{Deserialize, Deserializer};
use itertools::Itertools;

fn main() {
    let args = parse_args();

    // todo: be smarter about how we load these. It shouldn't need to wait for the request every time we launch it.
    let possible_templates = get_all_templates().expect("couldn't load the templates");

    match args.subcommand() {
        ("edit", Some(sub_m)) => {}
        ("get", Some(sub_m)) => {
            let requested_templates: Option<Vec<String>> = sub_m.args
                .get("templates")
                .map(|matched_arg| {
                    matched_arg.vals
                        .iter()
                        .filter_map(|os_string|
                            os_string.clone().into_string().ok())
                        .collect()
                });
        }
        _ => {}
    }
}

fn parse_args<'a>() -> ArgMatches<'a> {
    clap_app!(gig =>
        (setting: AppSettings::SubcommandRequiredElseHelp)
        (version: crate_version!())
        (author: crate_authors!("\n"))
        (about: "CLI interface to gitignore.io")
        (@subcommand get =>
            (about: "creates a new .gitignore for a given set of template keywords")
            (@arg templates: +multiple "User arguments")
        )
        (@subcommand edit =>
            (about: "edit which templates are included in a .gitignore")
        )
    ).get_matches()
}

fn do_get(maybe_requested_templates: Option<Vec<String>>, possible_templates: Vec<TemplateData>) {
    // if the user passed in some templates, just get those.
    // otherwise show the interactive templates picker
    match maybe_requested_templates {
        Some(requested_templates) => {}
        None => {}
    }
}

fn do_edit() { unimplemented!() }

#[derive(Debug, Deserialize)]
struct TemplateData {
    #[serde(rename = "fileName")] file_name: String,
    contents: String,
    name: String,
}

type RemoteTemplates = HashMap<String, TemplateData>;

fn get_all_templates() -> Result<RemoteTemplates, reqwest::Error> {
    let url = "https://www.gitignore.io/api/list?format=json";
    let mut templates: RemoteTemplates = match reqwest::get(url) {
        Ok(mut response) => match response.json() {
            Ok(json) => json,
            Err(err) => panic!("couldn't parse the response. Response: {:?}\nError: {}", response, err.description())
        },
        Err(err) => panic!("couldn't get the url: {}\nError: {}", url, err.description()),
    };
    Ok(templates)
}

type HeaderToIdMap = HashMap<String, String>;

fn make_header_to_id_map(all_template_data: RemoteTemplates) -> HeaderToIdMap {
    let is_header_regex = Regex::from_str("###.*###").unwrap();

    all_template_data
        .iter()
        .flat_map(|(template_id, template_data)| {
            template_data.contents.split('\n')
                .filter(|x| is_header_regex.is_match(x))
                .map(move |header| (header.to_string(), template_id.clone()))
        }).collect()
}

fn build_url_for_template(requested_template_ids: Vec<String>) -> Result<Url, reqwest::UrlError> {
    let base_url = "https://www.gitignore.io/api/";
    let ids = requested_template_ids.join(",");
    Url::from_str(&format!("{}{}", base_url, ids))
}

#[derive(Debug, Serialize)]
struct Gitignore {
    content_groups: Vec<Group>
}

impl Gitignore {
    fn from_readable(source_readable: &mut Read) -> Gitignore {
        let mut contents = String::new();
        source_readable.read_to_string(&mut contents);
        Self::from_string(contents)
    }

    fn from_string(source_string: String) -> Gitignore {
        let group_header_regex = Regex::new("###.*###").unwrap();
        let mut groups = vec![];
        for line in source_string.lines() {
            if group_header_regex.is_match(line) {
                groups.push(Group::with_header_text(line));
            } else {
                if let Some(ref mut group) = groups.last_mut() {
                    let line = Line::from_str(line);
                    let origin = None; // we'll determine the origin when we compare against the remote file
                    group.lines.push((line, origin));
                }
            }
        }
        Gitignore { content_groups: groups }
    }

    fn set_group_origins(&mut self, mapping: HeaderToIdMap) {
        use Origin::*;
        for (key, groups) in &self.content_groups.iter_mut().group_by(|group| mapping.get(&group.header_text)) {
            for group in groups {
                group.origin = match key {
                    Some(remote_id) => Remote { id: remote_id.clone() },
                    None => Local
                };
            }
        }
    }
}


#[derive(Debug, Serialize)]
struct Group {
    header_text: String,
    origin: Origin,
    lines: Vec<(Line, Option<Origin>)>
}

impl Group {
    fn with_header_text(header_text: &str) -> Group {
        Group {
            header_text: header_text.to_string(),
            origin: Origin::Unknown,
            lines: vec![]
        }
    }
}

#[derive(Debug, Serialize)]
enum Line {
    Whitespace,
    Comment { text: String },
    Entry { text: String }
}

impl Line {
    fn from_str(s: &str) -> Line {
        if s.trim().is_empty() {
            Line::Whitespace
        } else if s.trim().starts_with('#') {
            Line::Comment { text: s.to_string() }
        } else {
            Line::Entry { text: s.to_string() }
        }
    }
}

#[derive(Debug, Serialize)]
enum Origin {
    Local,
    Remote { id: String },
    Unknown // origin will be unknown until we get a chance to compare against the remote file. Is this necessary?
}

fn fetch_gitignore(url: Url) -> Result<Gitignore, reqwest::Error> {
    let mut resp = reqwest::get(url)?;
    let gitignore = Gitignore::from_readable(&mut resp);
    Ok(gitignore)
}


fn load_gitignore(dir_path: PathBuf) -> Result<Gitignore, std::io::Error> {
    let file_path = dir_path.clone().join("test.gitignore");
    let mut file = File::open(file_path)?;
    Ok(Gitignore::from_readable(&mut file))
}


#[cfg(test)]
mod tests {
    use super::*;

    fn get_test_directory() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("src") // there's got to be a better way to do this
            .join("resources")
            .join("test")
    }

    fn fake_get_all_templates() -> HashMap<String, TemplateData> {
        let file_path = get_test_directory().join("api_list_response.json");
        let mut file = File::open(file_path).unwrap();
        let mut contents = String::new();
        file.read_to_string(&mut contents);
        serde_json::from_str(&contents).unwrap()
    }

    fn setup_header_to_id_map() -> HeaderToIdMap {
        // real templates go here
        //         let templates = get_all_templates().unwrap();
        // fake templates. todo: mock this somehow
        let templates = fake_get_all_templates();
        make_header_to_id_map(templates)
    }

    #[test]
    fn can_load_test_gitignore() {
        let mut test_directory = get_test_directory();
        let gitignore = load_gitignore(test_directory).unwrap();
        assert!(gitignore.content_groups.len() > 0);
    }

    #[test]
    fn two_headers_in_the_same_template_map_to_the_same_key() {
        let header_to_id_map = setup_header_to_id_map();
        let header_1 = "### Intellij ###";
        let header_2 = "### Intellij Patch ###";
        let template_id = "intellij";
        let id_for_header_1 = header_to_id_map.get(header_1).unwrap();
        let id_for_header_2 = header_to_id_map.get(header_2).unwrap();
        assert_eq!(id_for_header_1, template_id);
        assert_eq!(id_for_header_2, template_id);
    }

    #[test]
    fn can_group_by_origin() {
        let header_to_id_map = setup_header_to_id_map();
        let mut test_directory = get_test_directory();
        let mut gitignore = load_gitignore(test_directory).unwrap();
        //        let group_by_origin = group_sections_by_origin(gitignore, header_to_id_map);
        gitignore.set_group_origins(header_to_id_map);
        //        println!("{}", serde_json::to_string(&group_by_origin).unwrap());
    }

    //#[test]
    //    fn test_something() {
    //        let url = build_url_for_template(vec![String::from("intellij")]).unwrap();
    //        println!("url: {}", url);
    //        let gitignore = fetch_gitignore(url).unwrap();
    //        println!("gitiginore: {:?}", gitignore);
    //    }
}