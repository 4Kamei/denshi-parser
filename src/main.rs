pub mod matcher;
pub mod syntax_matcher;

use crate::syntax_matcher::SyntaxMatcher;
use crate::syntax_matcher::{SyntaxItem, SyntaxItemType};

use clap::{Parser, Subcommand};
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
use std::rc::Rc;

use sv_parser::parse_sv_str;
use sv_parser::NodeEvent;
use sv_parser::RefNode;

use toml::Table;

use anyhow::Result;

use ansi_term::Colour;

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
    Colors,
    Find { regex: String },
}

fn main() -> Result<()> {
    let args = Args::parse();

    match args.command {
        Command::Parse | Command::Debug => {
            let _result = parse_groups(&args.config, &args.code, args.command == Command::Debug)?;
        }
        Command::Find { regex } => {
            find_regex(&args.code, &regex)?;
        }
        Command::Colors {} => {
            let _result = print_colors(&args.config)?;
        }
    }

    Ok(())
}

fn find_regex(code_path: &str, input_filter: &str) -> Result<()> {
    let code = &fs::read_to_string(code_path)?;

    let (tree, _) = parse_sv_str(
        code,
        PathBuf::from(code_path),
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

fn print_colors(toml_path: &str) -> Result<()> {
    //TODO would be good to separate out some of this, so that we don't need to create an entire
    //SyntaxMatcher just to print the colors

    let parsed_toml = &fs::read_to_string(toml_path)?.parse::<Table>()?;
    let mut matcher = SyntaxMatcher::from_toml(parsed_toml)?;

    for (color, string) in matcher.get_colors() {
        println!("{} {}", color, string);
    }

    Ok(())
}

fn parse_groups(toml_path: &str, code_path: &str, debug: bool) -> Result<()> {
    let mut code = fs::read_to_string(code_path)?;

    let mut backtick_indices = vec![];
    for (p, _) in code.char_indices().rev().filter(|(_, c)| c == &'`') {
        backtick_indices.push(p);
    }

    let mut compiler_directives = vec![];
    for p in backtick_indices {
        let line_end = code
            .char_indices()
            .skip(p)
            .take_while(|(_, c)| c != &'\n')
            .map(|(p, _)| p)
            .last()
            .unwrap_or(code.len() - 1);

        code.replace_range(p..line_end + 1, "");
        compiler_directives.push(SyntaxItem {
            group: "denshiCompilerDirective",
            col_start: p,
            col_end: line_end + 1,
            syntax_type: SyntaxItemType::Always,
        });
    }

    //Vivado, for example, uses (*  ...     *)  logic param_name; as a way to declare parameters on
    //nets. Treat these as comments. This is not ideal.
    code = code.replace("(*", "/*");
    code = code.replace("*)", "*/");

    let code = Rc::new(code);

    let result = parse_sv_str(
        &code,
        PathBuf::from(code_path),
        &HashMap::new(),
        &Vec::<PathBuf>::new(),
        false,
        false,
    );

    let parsed_toml = &fs::read_to_string(toml_path)?.parse::<Table>()?;
    let mut matcher = SyntaxMatcher::from_toml(parsed_toml)?;

    let mut breadcrumbs = vec![];

    let (tree, _) = result?;

    for node_event in tree.into_iter().event() {
        match node_event {
            NodeEvent::Enter(ref node) => {
                breadcrumbs.push(node.to_string());
                matcher.enter(node);
            }
            NodeEvent::Leave(ref node) => {
                breadcrumbs.pop();
                matcher.leave(node);
            }
        };
    }

    let group_colors = matcher.get_colors_as_ansi()?;

    let mut output_groups = matcher.compute(&code);
    if !debug {
        //Print the groups as input to the vim plugin
        print!(
            "{}",
            output_groups
                .iter()
                .map(|item| {
                    format!(
                        "{} {} {} {} {}",
                        item.group, item.line, item.col_start, item.col_end, item.matched
                    )
                })
                .collect::<Vec<_>>()
                .join("\n")
        );
    } else {
        for (group, c) in &group_colors {
            println!("Group: {}{}{}", c, group, "\x1b[0m");
        }

        let mut lines = code
            .split("\n")
            .map(|x| x.to_string())
            .collect::<Vec<String>>();
        output_groups.sort_by(|a, b| usize::cmp(&b.col_start, &a.col_start));
        output_groups.sort_by(|a, b| usize::cmp(&b.line, &a.line));
        dbg!(&output_groups);
        for item in output_groups {
            lines
                .get_mut(item.line - 1)
                .context(format!("Could not get line for item {:?}", item))?
                .replace_range(
                    item.col_start..item.col_end,
                    format!(
                        "{}{}{}",
                        group_colors
                            .get(item.group)
                            .context(format!("Could not find color for group {}", item.group))?,
                        item.matched,
                        "\x1b[0m"
                    )
                    .as_ref(),
                );
        }
        println!("{}", lines.join("\n"));
    }
    Ok(())
}

use anyhow::Context;
