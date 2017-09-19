//! WIP: An over-the-top gitignore.io command-line interface
//! With typeahead search and other goodies
extern crate reqwest;
extern crate regex;
extern crate termion;
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
use std::io::{self, Read, Write, stdout, Stdin, Stdout, StdoutLock};
use std::str::FromStr;
use std::string::ParseError;
use std::error::Error;
use std::fs::File;
use std::path::{PathBuf, Path};
use std::collections::HashMap;
use serde_json::Value;
use serde::{Deserialize, Deserializer};
use itertools::Itertools;
use std::thread;
use std::sync::mpsc::{Sender, Receiver};
use std::sync::mpsc;
use termion::raw::{IntoRawMode, RawTerminal};
use termion::input::TermRead;
use termion::{color, style, cursor};

fn main() {
    let args = parse_args();

    // todo: be smarter about how we load these. It shouldn't need to wait for the request every time we launch it.
    let possible_templates = get_all_templates().expect("couldn't load the templates");

    match args.subcommand() {
        ("edit", Some(sub_m)) => {
            do_edit();
        }
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

#[derive(Debug, Clone)]
enum RequestType {
    Templates(RemoteTemplates)
}

#[derive(Debug, Clone)]
enum EditEvent {
    Key(termion::event::Key),
    RequestComplete(RequestType),
    Break
}

fn async_get_all_templates(outbox: Sender<EditEvent>) {
    use EditEvent::*;
    use RequestType::*;
    thread::spawn(move || {
        let templates = get_all_templates();
        if let Ok(templates) = templates {
            outbox.send(RequestComplete(Templates(templates)));
        }
    });
}

struct EditState {
    highlighted_row: i32,
    row_open: bool
}

impl EditState {
    fn new() -> Self {
        EditState {
            highlighted_row: 0,
            row_open: false
        }
    }

    fn move_selection_up(&mut self) {
        self.highlighted_row += 1;
    }

    fn move_selection_down(&mut self) {
        self.highlighted_row -= 1;
    }

    fn open_selection(&mut self) {
        self.row_open = true;
    }

    fn close_selection(&mut self) {
        self.row_open = false;
    }
}

fn do_edit() {
    use termion::event::Key::*;
    use EditEvent::*;
    use RequestType::*;

    let (events_outbox, events_inbox) = mpsc::channel::<EditEvent>();


    let stdout = stdout();
    let mut wrapion = Wrapion::new(stdout.lock());
    async_get_all_templates(events_outbox.clone());
    wrapion.async_key_events(events_outbox.clone());

    let mut state = EditState::new();

    wrapion.clear();
    draw_gui(&mut wrapion);
    // note, suspect that println! will deadlock since we took out a permanent lock on stdout
    loop {
        let event = events_inbox.recv();
        wrapion.println(&format!("{:?}", event));
        match event.unwrap_or(Break) {
            Key(key) => match key {
                Up => state.move_selection_up(),
                Down => state.move_selection_down(),
                Left => state.close_selection(),
                Right => state.open_selection(),
                _ => {}
            },
            RequestComplete(request_type) => match request_type {
                Templates(templates) => update_templates(templates)
            },
            Break => { break }
        }

        draw_gui(&mut wrapion);
    }
}

fn draw_gui<W: Write>(wrapion: &mut Wrapion<W>) {
    wrapion.println("yeah");
}

struct Wrapion<W: Write> {
    _guard: termion::PreInitState,
    stdout: W,
}

impl<W: Write> Wrapion<W> {
    fn new(stdout: W) -> Wrapion<RawTerminal<W>> {
        Wrapion {
            _guard: termion::init(),
            stdout: stdout.into_raw_mode().unwrap(),
        }
    }

    fn println(&mut self, output: &str) {
        writeln!(self.stdout, "{}", output);
        self.stdout.flush();
    }

    fn async_key_events(&mut self, f: Sender<EditEvent>) {
        use termion::async_stdin;
        thread::spawn(move || {
            let stdin = std::io::stdin();
            let mut keys = stdin.lock().keys();
            loop {
                let key_result = keys.next();
                if let Some(Ok(key)) = key_result {
                    f.send(EditEvent::Key(key));
                }
            }
        });
    }

    fn clear(&mut self) {
        write!(self.stdout, "{}{}yo yo yo",
               termion::clear::All,
               termion::cursor::Goto(1, 1))
            .unwrap();
        self.stdout.flush();
    }
}

impl<W: Write> Drop for Wrapion<W> {
    fn drop(&mut self) {
        write!(self.stdout, "{}{}{}{}",
               color::Bg(color::Reset),
               color::Fg(color::Reset),
               style::Reset,
               cursor::Show
        );
        self.stdout.flush();
    }
}

fn update_templates(templates: RemoteTemplates) {}

#[derive(Debug, Deserialize, Clone)]
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
    let group_header_regex = Regex::from_str("###.*###").unwrap();
    all_template_data
        .iter()
        .flat_map(|(template_id, template_data)| {
            template_data.contents.split('\n')
                .filter(|x| group_header_regex.is_match(x))
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
    fn local<T: Into<Gitignore>>(source: T) -> Self {
        let mut gitignore = source.into();
        gitignore.uniform_origin(Origin::Local);
        gitignore
    }

    fn uniform_origin(&mut self, origin: Origin) {
        for group in self.content_groups.iter_mut() {
            group.origin = origin.clone();
            for &mut (_, ref mut line_origin) in group.lines.iter_mut() {
                *line_origin = origin.clone();
            }
        }
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

    // big picture. the point of this is for editing a gitignore:
    // we list the groups, each is marked with a flag (Local, Remote, or Mixed) based on what we determine here
    // when we go to delete a group, if it is Mixed, we can then prompt the user to see if they want to
    // either move the changes to a new group, or delete the changes, or cancel the delete.
    fn compute_diff_against_remote(&mut self, other: &Gitignore) {
        // todo: rethink this
        // for each of our groups
        //   let `other_group` = the corresponding group in `other` if it exists (the corrosponding group is by id)
        //     for each of our lines where the type is `Entry`
        //       find the corresponding line in `other_group` if it exists
        //         if it exists, mark our line's `origin` as `Remote {id}` where id is ?
        //         otherwise mark our line's `origin` as `Local`

        fn find_corrosponding_group<'a>(group: &Group, others: &'a Vec<Group>) -> Option<&'a Group> {
            others.iter().find(|x| x.origin == group.origin)
        }

        fn find_corrosponding_line<'a>(line: &Line, others: &'a Vec<Line>) -> Option<&'a Line> {
            others.iter().find(|x| *x == line)
        }

        for our_group in self.content_groups.iter_mut() {
            let maybe_other_group = find_corrosponding_group(&our_group, &other.content_groups);
            println!("other_group: {:?}", maybe_other_group);
            if let Some(ref other_group) = maybe_other_group {
                for our_line in our_group.lines.iter_mut() {
                    match our_line.0 {
                        Line::Entry { .. } => {
                            let bb = other_group.lines
                                .iter()
                                .map(|&(ref line, _)| line.clone())
                                .collect();
                            let maybe_other_line = find_corrosponding_line(&our_line.0, &bb);
                            match maybe_other_line {
                                Some(other_line) => { our_line.1 = Origin::Remote { id: String::new() } } // todo: what goes here? This doesn't make a lot of sense.
                                _ => { our_line.1 = Origin::Local }
                            }
                        }
                        _ => {}
                    }
                }
            } else {
                // if it doesn't exist, all the lines must be local
                for &mut (_, ref mut origin) in our_group.lines.iter_mut() {
                    *origin = Origin::Local;
                }
            }
        }
    }
}

impl<T: Read> From<T> for Gitignore {
    fn from(mut source: T) -> Self {
        let mut source_string = String::new();
        source.read_to_string(&mut source_string);

        let group_header_regex = Regex::new("###.*###").unwrap();
        let mut groups = vec![];
        for line in source_string.lines() {
            if group_header_regex.is_match(line) {
                groups.push(Group::with_header_text(line));
            } else {
                if let Some(ref mut group) = groups.last_mut() {
                    let line = Line::from_str(line);
                    group.lines.push((line, Origin::Unknown));
                }
            }
        }
        Gitignore { content_groups: groups }
    }
}

#[derive(Debug, Serialize)]
struct Group {
    header_text: String,
    origin: Origin,
    lines: Vec<(Line, Origin)>
}

impl Group {
    fn with_header_text(header_text: &str) -> Self {
        Group {
            header_text: header_text.to_string(),
            origin: Origin::Unknown,
            lines: vec![]
        }
    }
}

#[derive(Debug, Serialize, Clone, Eq, PartialEq)]
enum Line {
    Whitespace,
    Comment { text: String },
    Entry { text: String }
}

impl Line {
    fn from_str(s: &str) -> Self {
        if s.trim().is_empty() {
            Line::Whitespace
        } else if s.trim().starts_with('#') {
            Line::Comment { text: s.to_string() }
        } else {
            Line::Entry { text: s.to_string() }
        }
    }
}

#[derive(Debug, Serialize, PartialEq, Eq, Hash, Clone)]
enum Origin {
    Local,
    Remote { id: String },
    // id is the gitignore.io id for lines and groups
    Unknown // origin will be unknown until we get a chance to compare against the remote file. Should we default to Local?
}

//enum LineOrigin {
//    Remote,
//    Local
//}

fn fetch_gitignore(url: Url) -> Result<Gitignore, reqwest::Error> {
    let mut resp = reqwest::get(url)?;
    Ok(Gitignore::from(resp))
}

fn load_gitignore(dir_path: PathBuf) -> Result<Gitignore, std::io::Error> {
    let file_path = dir_path.clone().join(".gitignore");
    let file = File::open(file_path)?;
    Ok(Gitignore::local(file))
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
        gitignore.set_group_origins(header_to_id_map.clone()); //todo: bad!
        let remote_ids = gitignore.content_groups.iter()
            .map(|group| &group.origin)
            .unique()
            .filter_map(|origin| match origin {
                &Origin::Remote { ref id } => Some(id.clone()),
                _ => None
            }).collect();
        let url = build_url_for_template(remote_ids).unwrap();
        println!("url: {:?}", url);
        let mut remote_gitignore = fetch_gitignore(url).unwrap();
        remote_gitignore.set_group_origins(header_to_id_map);
        println!("{}", serde_json::to_string(&remote_gitignore).unwrap());
        gitignore.compute_diff_against_remote(&remote_gitignore);
        println!("{}", serde_json::to_string(&gitignore).unwrap());
    }
}