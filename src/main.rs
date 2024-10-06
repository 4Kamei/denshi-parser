pub mod matcher;

use crate::matcher::{BreadcrumbsMatcher, MatchPattern};
use clap::{Parser, Subcommand};
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
use std::rc::Rc;
use sv_parser::parse_sv_str;
use sv_parser::NodeEvent;
use sv_parser::RefNode;
use toml::Table;

/// Parser for the associated nvim plugin for systemverilog syntax highlighting
#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Args {
    code: String,
    config: String,

    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand, PartialEq)]
enum Command {
    Parse,
    Debug,
    Find { regex: String },
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = Args::parse();

    match args.command {
        Command::Parse | Command::Debug => {
            let _result = parse_groups(&args.config, &args.code, args.command == Command::Debug)?;
        }
        Command::Find { regex } => {
            find_regex(&args.code, &regex)?;
        }
    }

    Ok(())
}
fn index_to_line_col(s: &str, index: usize) -> Option<(usize, usize)> {
    if index >= s.len() {
        return None; // Index is out of bounds
    }

    let mut line = 0;
    let mut char_count = 0;

    for line_content in s.lines() {
        let line_length = line_content.len();

        // Check if the index is within the current line
        if char_count + line_length >= index {
            return Some((line + 1, index - char_count + 1)); // 1-based indexing
        }

        char_count += line_length + 1; // +1 for the newline character
        line += 1;
    }

    None // This should not be reached due to the initial bounds check
}

fn find_regex(code_path: &str, input_filter: &str) -> Result<(), Box<dyn std::error::Error>> {
    let code = &fs::read_to_string(code_path)?;

    let (tree, _) = parse_sv_str(
        code,
        PathBuf::from("test.sv"),
        &HashMap::new(),
        &Vec::<PathBuf>::new(),
        false,
        false,
    )?;

    let mut breadcrumbs = vec![];

    for node_event in tree.into_iter().event() {
        match node_event {
            NodeEvent::Enter(ref node) => {
                breadcrumbs.push(node.to_string());
            }
            NodeEvent::Leave(ref _node) => {
                breadcrumbs.pop();
            }
        };

        if let NodeEvent::Enter(RefNode::Locate(locate)) = node_event {
            let name = locate.str(code);
            if name == input_filter {
                println!(
                    "\nLine: {}\n{}",
                    code.lines()
                        .nth((locate.line - 1).try_into().unwrap())
                        .unwrap(),
                    breadcrumbs
                        .clone()
                        .into_iter()
                        .collect::<Vec<_>>()
                        .join(" ")
                );
            }
        }
    }

    Ok(())
}

fn parse_groups(
    toml_path: &str,
    code_path: &str,
    debug: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    let toml = &fs::read_to_string(toml_path)?.parse::<Table>()?;

    let mut map: HashMap<String, String> = HashMap::new();

    for (name, value) in toml.iter() {
        match value {
            toml::Value::Table(inner) => {
                for (i_name, i_value) in inner.iter() {
                    if let toml::Value::String(param_value) = i_value {
                        if i_name == "group" {
                            map.insert(name.to_string(), param_value.to_string());
                        }
                    }
                }
            }
            other => println!("Unparsed {:?}", other),
        }
    }

    let code = fs::read_to_string(code_path)?;
    let code = Rc::new(code);

    let result = parse_sv_str(
        &code,
        PathBuf::from("test.sv"),
        &HashMap::new(),
        &Vec::<PathBuf>::new(),
        false,
        false,
    );

    let mut matchers = vec![];

    for (filter, group) in map.iter() {
        let filter_split = filter.split(" ");
        let cl_group = group.clone();
        let inner_code = Rc::clone(&code);
        let cmd = move |locate: &sv_parser::Locate| {
            let line = locate.line;
            let col_start = locate.offset;
            let col_end = locate.offset + locate.len;

            let (line_start, col_start) = index_to_line_col(&inner_code, col_start)
                .expect("Starting column should be within code");
            let (line_end, col_end) = index_to_line_col(&inner_code, col_end)
                .expect("Ending column should be within code");

            assert!(
                line_start == line_end,
                "Starting and ending line should be the same"
            );

            if debug {
                println!(
                    "{} {line} {col_start} {col_end}    \t: ({})",
                    cl_group,
                    locate.str(&inner_code)
                );
            } else {
                println!(
                    "{} {line} {col_start} {col_end} {}",
                    cl_group,
                    locate.str(&inner_code)
                );
            };
        };
        let mut filter_match = vec![];
        for k in filter_split.into_iter() {
            let first_char = &k[0..1]; //.expect("Syntax identifier should be longer than 0");
            if first_char == "^" {
                filter_match.push(MatchPattern::NotMatches(k[1..].into()));
            } else {
                filter_match.push(MatchPattern::Matches(k.into()));
            }
        }
        let bc = BreadcrumbsMatcher::new(filter_match, Box::new(cmd));
        matchers.push(bc)
    }
    let mut breadcrumbs = vec![];

    let (tree, _) = result?;

    for node_event in tree.into_iter().event() {
        match node_event {
            NodeEvent::Enter(ref node) => {
                breadcrumbs.push(node.to_string());
                for m in &mut matchers {
                    m.enter(node);
                }
            }
            NodeEvent::Leave(ref node) => {
                breadcrumbs.pop();
                for m in &mut matchers {
                    m.leave(node);
                }
            }
        };

        //println!(
        //   "",
        //    breadcrumbs
        //        .iter()
        //        .map(|x| x.into())
        //        .collect::<Vec<String>>()
        //        .join(" ")
        //);
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    enum Event<'a> {
        Enter(&'a str),
        Leave(&'a str),
    }

    #[test]
    fn matcher_respects_not_equal() {
        let stim: Vec<(bool, Event)> = vec![
            (false, Event::Enter("Base")),
            (false, Event::Enter("NextLevel")),
            (false, Event::Enter("DisallowedLevel")),
            (false, Event::Enter("Something")),
            (false, Event::Leave("Something")),
            (false, Event::Leave("DisallowedLevel")),
        ];

        let pattern = vec![
            MatchPattern::Matches("Base"),
            MatchPattern::NotMatches("DisallowedLevel"),
            MatchPattern::Matches("Something"),
        ];

        let mut bc = BreadcrumbsMatcher::new(pattern, Box::new(|_| {}));

        for (matches, event) in stim {
            match event {
                Event::Enter(e) => bc.enter(&e),
                Event::Leave(e) => bc.leave(&e),
            }
            assert!(
                bc.matches() == matches,
                "Incorrectly computed matched/mismatch"
            );
        }
    }
}
