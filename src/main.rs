#[macro_use]
extern crate clap;
extern crate reqwest;
extern crate serde;
#[macro_use]
extern crate serde_derive;
extern crate termion;

use clap::{AppSettings, ArgMatches};
use reqwest::Url;
use std::fs;
use std::io::{stdin, stdout, Stdout, Write};
use std::str::FromStr;
use termion::clear;
use termion::color;
use termion::cursor;
use termion::event::Key;
use termion::input::TermRead;
use termion::raw::IntoRawMode;
use termion::raw::RawTerminal;

fn main() {
    match parse_args().subcommand() {
        ("edit", Some(_)) => do_edit(),
        ("get", Some(matches)) => do_get(matches),
        _ => {}
    }
}

fn parse_args<'a>() -> ArgMatches<'a> {
    clap_app!(gig =>
        (setting: AppSettings::SubcommandRequiredElseHelp)
        (version: crate_version!())
        (author: crate_authors!("\n"))
        (about: "CLI .gitignore manager")
        (@subcommand get =>
            (about: "creates a new .gitignore for a given set of template keywords")
            (@arg templates: +multiple "User arguments")
        )
        (@subcommand edit =>
            (about: "edit which templates are included in a .gitignore")
        )
    ).get_matches()
}

fn do_edit() { panic!("not implemented"); }

fn do_get(args: &ArgMatches) {
    let requested_templates = get_requested_templates(args);
    let downloaded: Option<String> = match requested_templates {
        Some(templates) => download_gitignore(templates).expect("couldn't download gitignore").into(),
        None => interactive_get()
    };

    if let Some(gitignore) = downloaded {
        merge_or_create_gitignore(gitignore);
    } else {
        println!("no action taken");
    }
}

fn download_gitignore(template_ids: Vec<String>) -> Result<String, reqwest::Error> {
    println!("downloading templates {:?}", template_ids);
    let base_url = "https://www.gitignore.io/api/";
    let ids = template_ids.join(",");
    let url = Url::from_str(&format!("{}{}", base_url, ids)).unwrap();
    let result = reqwest::get(url)?.text()?;
    Ok(result)
}

fn interactive_get() -> Option<String> {
    println!("fetching list of possible keywords...");
    let ids = get_possible_ids();
    let choices = interactive_choose_ids(ids);
    if choices.is_empty() {
        println!("nothing selected");
        return None;
    }
    let gitignore = download_gitignore(choices).expect("couldn't download gitignore");
    Some(gitignore)
}

fn interactive_choose_ids(ids: Vec<String>) -> Vec<String> {
    let mut selections = Vec::new();
    let mut continue_with = None;

    fn print_desc(stdout: &mut RawTerminal<Stdout>, selections: &Vec<String>) -> Result<(), std::io::Error> {
        let mut i = 1..;
        write!(stdout, "{}{}selected: {}", clear::All, cursor::Goto(1, i.next().unwrap()), selections.join(", "))?;
        write!(stdout, "{}keep typing to add more", cursor::Goto(1, i.next().unwrap()))?;
        write!(stdout, "{}press backspace to delete", cursor::Goto(1, i.next().unwrap()))?;
        write!(stdout, "{}press return to accept the current set.", cursor::Goto(1, i.next().unwrap()))?;
        stdout.flush()?;
        Ok(())
    }

    loop {
        // todo: pass the set difference between 'ids' and 'selections'
        match fuzzy_choose(&ids, &continue_with) {
            Some(id) => {
                selections.push(id);
                let stdin = stdin();
                let mut stdout = stdout().into_raw_mode().unwrap();
                print_desc(&mut stdout, &selections).unwrap();
                let mut stdin = stdin.lock().keys();
                loop {
                    match stdin.next().unwrap().unwrap() {
                        Key::Char('\n') => {
                            return selections;
                        }
                        Key::Backspace => { selections.pop(); }
                        Key::Char(c) => {
                            continue_with = Some(c.to_string());
                            break;
                        }
                        _ => continue
                    }
                    print_desc(&mut stdout, &selections).unwrap();
                }
                write!(stdout, "{}{}", cursor::Goto(1, 1), clear::All).unwrap();
            }
            None => {
                println!("no selection made");
                break;
            }
        }
    }
    selections
}

fn fuzzy_choose(available: &Vec<String>, continue_with: &Option<String>) -> Option<String> {
    fn display_matches(stdout: &mut RawTerminal<Stdout>,
                       search_string: &String,
                       items_to_display: &Vec<(usize, String)>,
                       selection: Option<usize>) {
        write!(stdout, "{}", clear::All).unwrap();
        for (i, tup) in items_to_display.iter().enumerate() {
            let &(_, ref string) = tup;
            if i == selection.unwrap_or(0) {
                write!(stdout, "{}{}{}> {}{}{}",
                       cursor::Goto(1, (i + 2) as u16),
                       color::Bg(color::White),
                       color::Fg(color::Black),
                       string,
                       color::Bg(color::Reset),
                       color::Fg(color::Reset)
                ).unwrap();
            } else {
                write!(stdout, "{}> {}", cursor::Goto(1, (i + 2) as u16), string).unwrap();
            }
        }
        write!(stdout, "{} search_str:: {}", cursor::Goto(1, 1), search_string).unwrap();
        stdout.flush().unwrap();
    }

    fn find_matches(available: &Vec<String>, search_string: &String) -> Vec<(usize, String)> {
        let mut items_to_display = Vec::new();
        for item in available.iter() {
            if let Some(match_start_pos) = item.to_lowercase().find(&search_string.to_lowercase()) {
                items_to_display.push((match_start_pos, item.clone()));
            }
        }
        items_to_display.sort_by_key(|x| x.0);
        items_to_display
    }

    let stdin = stdin();
    let mut stdout = stdout().into_raw_mode().unwrap();
    stdout.flush().unwrap();

    let selection;
    let mut search_string = continue_with.clone().unwrap_or_default();
    let mut keys = stdin.lock().keys();
    let mut interior_selection: Option<usize> = None;
    loop {
        let items_to_display = find_matches(available, &search_string);
        display_matches(&mut stdout, &search_string, &items_to_display, interior_selection);

        let key = keys.next().unwrap().unwrap();
        match key {
            Key::Char('\n') => {
                selection = items_to_display.get(interior_selection.unwrap_or(0)).cloned();
                break;
            }
            Key::Char(c) => {
                interior_selection = None;
                search_string.push(c);
            }
            Key::Backspace => {
                interior_selection = None;
                search_string.pop();
            }
            Key::Down => {
                if interior_selection.unwrap_or_default() == items_to_display.len() - 1 {
                    interior_selection = Some(0);
                } else {
                    interior_selection = Some(interior_selection.unwrap_or_default() + 1);
                }
            }
            Key::Up => {
                if items_to_display.len() == 0 {
                    interior_selection = None;
                } else if interior_selection.unwrap_or_default() == 0 {
                    interior_selection = Some(items_to_display.len() - 1);
                } else {
                    interior_selection = Some(interior_selection.unwrap_or_default() - 1);
                }
            }
            Key::Ctrl('c') => {
                write!(stdout, "{}{}", cursor::Goto(1, 1), clear::All).unwrap();
                return None;
            }
            _ => {}
        };
    }

    write!(stdout, "{}{}", cursor::Goto(1, 1), clear::All).unwrap();
    selection.map(|x| x.1)
}

fn get_possible_ids() -> Vec<String> {
    let url = "https://www.gitignore.io/api/list";
    reqwest::get(url)
        .expect("couldn't get the list of ids")
        .text()
        .expect("couldn't read the file")
        .split('\n')
        .flat_map(|x| x.split(','))
        .map(str::trim)
        .map(str::to_string)
        .collect()
}

fn merge_or_create_gitignore(gitignore: String) {
    if current_directory_has_gitignore() {
        println!(".gitignore detected in working directory");
        let should_merge = read_yes_or_no("merge? [Y/n]");
        if should_merge {
            println!("ok")
        } else {
            println!("aborting")
        }
    } else {
        // create
        let mut file = fs::File::create("./.gitignore").expect("couldn't create file");
        match file.write_all(gitignore.as_bytes()) {
            Ok(()) => println!("created .gitignore"),
            Err(e) => eprintln!("{}", e)
        }
    }
}

fn current_directory_has_gitignore() -> bool {
    let current_dir_path = std::env::current_dir().expect("Couldn't read current directory");
    match current_dir_path.read_dir() {
        Ok(paths) => {
            if paths_list_contains_gitignore(paths) {
                return true;
            }
        }
        Err(why) => println!("Couldn't list files in directory {:?}, {:?}", current_dir_path, why.kind()),
    }
    false
}

fn paths_list_contains_gitignore(paths: fs::ReadDir) -> bool {
    for maybe_path in paths {
        if let Ok(path) = maybe_path {
            if path.file_name().eq(".gitignore") {
                return true;
            }
        }
    }
    false
}

fn read_yes_or_no(msg: &str) -> bool {
    let stdin = stdin();
    let mut stdout = stdout().into_raw_mode().unwrap();
    write!(stdout, "{}", msg).unwrap();
    stdout.flush().unwrap();
    let key = stdin.keys().next().unwrap().unwrap();
    let result = match key {
        Key::Char('y') | Key::Char('Y') | Key::Char('\n') => true,
        _ => false
    };
    write!(stdout, "{}", cursor::Right(0)).unwrap();
    result
}

fn get_requested_templates(matches: &ArgMatches) -> Option<Vec<String>> {
    return matches.args
        .get("templates")
        .map(|matched_arg| {
            matched_arg.vals
                .iter()
                .filter_map(|os_string|
                    os_string.clone().into_string().ok())
                .collect()
        });
}


//fn get_all_templates() -> Result<RemoteTemplates, reqwest::Error> {
//    let url = "https://www.gitignore.io/api/list?format=json";
//    let templates = match reqwest::get(url) {
//        Ok(mut response) => match response.json() {
//            Ok(json) => json,
//            Err(err) => panic!("couldn't parse the response. Response: {:?}\nError: {}", response, err)
//        },
//        Err(err) => panic!("couldn't get the url: {}\nError: {}", url, err),
//    };
//    Ok(templates)
//}

#[derive(Debug, Deserialize, Clone)]
struct TemplateData {
    #[serde(rename = "fileName")] file_name: String,
    contents: String,
    name: String,
}

//type RemoteTemplates = HashMap<String, TemplateData>;
//
//mod gitignore {
//    #[derive(Debug, Serialize)]
//    struct Gitignore {
//        content_groups: Vec<Group>
//    }
//
//    #[derive(Debug, Serialize)]
//    struct Group {
//        header_text: String,
//        origin: Origin,
//        lines: Vec<(Line, Origin)>,
//    }
//
//    #[derive(Debug, Serialize, Clone, Eq, PartialEq)]
//    enum Line {
//        Whitespace,
//        Comment { text: String },
//        Entry { text: String },
//    }
//
//    #[derive(Debug, Serialize, PartialEq, Eq, Hash, Clone)]
//    enum Origin {
//        Local,
//        Remote { id: String },
//        // id is the gitignore.io id for lines and groups
//        Unknown, // origin will be unknown until we get a chance to compare against the remote file. Should we default to Local?
//    }
//}
