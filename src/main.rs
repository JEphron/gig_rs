//! WIP: An over-the-top gitignore.io command-line interface
//! With typeahead search and other goodies
#[macro_use]
extern crate clap;
extern crate reqwest;
extern crate regex;
extern crate serde;
extern crate serde_json;
#[macro_use]
extern crate serde_derive;

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
            //            do_get(requested_templates, possible_templates);
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
    //    unimplemented!()
}

fn do_get(maybe_requested_templates: Option<Vec<String>>, possible_templates: Vec<TemplateData>) {
    // if the user passed in some templates, just get those.
    // otherwise show the interactive templates picker
    match maybe_requested_templates {
        Some(requested_templates) => {}
        None => {}
    }
}

fn build_url_for_template(requested_template_ids: Vec<String>) -> Result<Url, reqwest::UrlError> {
    let base_url = "https://www.gitignore.io/api/";
    let ids = requested_template_ids.join(",");
    Url::from_str(&format!("{}{}", base_url, ids))
}

#[derive(Debug)]
struct Gitignore {
    sections: HashMap<String, Section>
}

impl Gitignore {
    fn from_string(contents: String) -> Gitignore {
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
        Gitignore { sections: sections }
    }
}

#[derive(Debug, Serialize)]
struct Section {
    subsections: Vec<String>
}

fn fetch_gitignore(url: Url) -> Result<Gitignore, reqwest::Error> {
    let mut resp = reqwest::get(url)?;
    let mut contents = String::new();
    resp.read_to_string(&mut contents);
    let gitignore = Gitignore::from_string(contents);
    Ok(gitignore)
}

fn do_edit() { unimplemented!() }

fn make_header_to_id_map(all_template_data: RemoteTemplates) -> HashMap<String, String> {
    let is_header_regex = Regex::from_str("### .* ###").unwrap();

    all_template_data
        .iter()
        .flat_map(|(template_id, template_data)| {
            template_data.contents.split('\n')
                .filter(|x| is_header_regex.is_match(x))
                .map(move |header| (header.trim_matches('#').trim().to_string(), template_id.clone()))
        }).collect()
}


fn load_gitignore(dir_path: PathBuf) -> Result<Gitignore, std::io::Error> {
    let file_path = dir_path.clone().join("test.gitignore");
    println!("{}", file_path.display());
    let mut file = File::open(file_path)?;
    let mut contents = String::new();
    file.read_to_string(&mut contents)?;
    Ok(Gitignore::from_string(contents))
}

enum SectionType {
    Remote,
    Local
}

// we want to see which headers are legit in the gitignore.io remote thingy and which are local
fn group_sections_by_origin(gitignore: Gitignore, mapping: HashMap<String, String>)
                            -> HashMap<String, Vec<Section>> {
    let as_vec: Vec<(String, Section)> = gitignore.sections.into_iter().collect();
    let grouped = into_group_by(as_vec,
                                |&(ref header, _)| {
                                    println!("{}", header);
                                    match mapping.get(header) {
                                        Some(id) => id,
                                        None => "local"
                                    }
                                });
    grouped.into_iter().map(|(x, y)|
        (x.to_string(),
         y
             .into_iter()
             .map(|z| z.1)
             .collect::<Vec<Section>>())).collect()
}

fn group_by<'a, TValue, TKey, TFunc>(list: &'a Vec<TValue>, key: TFunc) -> HashMap<TKey, Vec<&'a TValue>>
    where TFunc: Fn(&TValue) -> TKey,
          TKey: Eq + std::hash::Hash {
    let mut groups: HashMap<TKey, Vec<&'a TValue>> = HashMap::new();
    for item in list {
        groups.entry(key(&item)).or_insert(vec![&item]).push(item);
    }
    return groups;
}


fn into_group_by<TValue, TKey, TFunc>(list: Vec<TValue>, key: TFunc) -> HashMap<TKey, Vec<TValue>>
    where TFunc: Fn(&TValue) -> TKey,
          TKey: Eq + std::hash::Hash {
    let mut groups: HashMap<TKey, Vec<TValue>> = HashMap::new();
    for item in list.into_iter() {
        groups.entry(key(&item)).or_insert(vec![]).push(item);
    }
    return groups;
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

    fn setup_header_to_id_map() -> HashMap<String, String> {
        /// real templates go here
        //         let templates = get_all_templates().unwrap();
        /// fake templates
        let templates = fake_get_all_templates();
        make_header_to_id_map(templates)
    }

    #[test]
    fn can_load_test_gitignore() {
        let mut test_directory = get_test_directory();
        let gitignore = load_gitignore(test_directory).unwrap();
        assert!(gitignore.sections.len() > 0);
    }

    #[test]
    fn two_headers_in_the_same_template_map_to_the_same_key() {
        let header_to_id_map = setup_header_to_id_map();
//        let header_1 = "### Intellij ###";
//        let header_2 = "### Intellij Patch ###";
        let header_1 = "Intellij";
        let header_2 = "Intellij Patch";
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
        let gitignore = load_gitignore(test_directory).unwrap();
        let group_by_origin = group_sections_by_origin(gitignore, header_to_id_map);
        println!("{}", serde_json::to_string(&group_by_origin).unwrap());
    }

    //#[test]
    //    fn test_something() {
    //        let url = build_url_for_template(vec![String::from("intellij")]).unwrap();
    //        println!("url: {}", url);
    //        let gitignore = fetch_gitignore(url).unwrap();
    //        println!("gitiginore: {:?}", gitignore);
    //    }
}