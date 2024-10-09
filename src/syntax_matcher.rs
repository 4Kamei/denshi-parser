use crate::matcher::{BreadcrumbsMatcher, MatchPattern};
use clap::{Parser, Subcommand};
use std::collections::HashMap;
use std::collections::HashSet;
use std::fs;
use std::path::PathBuf;
use std::rc::Rc;
use sv_parser::parse_sv_str;
use sv_parser::NodeEvent;
use sv_parser::RefNode;
use toml::Table;

use anyhow::bail;
use anyhow::Context;
use std::cell::RefCell;

#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash)]
pub enum SyntaxItemType<'a> {
    Always,
    //The group to check with, to see if the text we matched is also defined there
    IfDefined(&'a str),
    IfDefinedElse(&'a str, &'a str),
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct SyntaxItem<'a> {
    pub group: &'a str,
    pub col_start: usize, //  Treating the whole code as 1 line
    pub col_end: usize,   //  Treating the whole code as 1 line
    pub syntax_type: SyntaxItemType<'a>,
}

#[derive(Debug)]
pub struct MatchedSyntaxItem<'a> {
    pub group: &'a str,
    pub line: usize,
    pub col_start: usize, //  Treating the whole code as 1 line
    pub col_end: usize,   //  Treating the whole code as 1 line
    pub matched: &'a str,
}

impl<'a> MatchedSyntaxItem<'a> {
    fn from_unmatched(item: SyntaxItem<'a>, code: &'a str) -> Vec<Self> {
        let line_vec =
            MatchedSyntaxItem::range_to_lines_cols(item.col_start, item.col_end - 1, &code);

        assert!(
            line_vec.len() > 0,
            "Must return more lines than 0? {}, {}",
            item.col_start,
            item.col_end
        );

        let mut out = vec![];

        for (line_number, col_start, col_end) in line_vec {
            let matched = &code.lines().nth(line_number).unwrap()[col_start..col_end];

            out.push(Self {
                group: item.group,
                matched,
                col_start,
                col_end,
                line: line_number + 1,
            });
        }
        return out;
    }

    fn range_to_lines_cols(
        start_col: usize,
        end_col: usize,
        code: &str,
    ) -> Vec<(usize, usize, usize)> {
        let mut output: Vec<(usize, usize, usize)> = vec![];
        let mut found_start = false;
        let mut char_count = 0;
        for (index, line) in code.lines().enumerate() {
            let line_len = line.len();
            if !found_start {
                if line_len + char_count > end_col {
                    return vec![(index, start_col - char_count, end_col + 1 - char_count)];
                }
                if line_len + char_count > start_col {
                    found_start = true;
                    output.push((index, start_col - char_count, line_len));
                }
            } else {
                if line_len + char_count > end_col {
                    output.push((index, 0, end_col + 1 - char_count));
                    return output;
                } else {
                    output.push((index, 0, line_len));
                }
            }
            char_count += line_len + 1;
        }
        return output;
    }
}

pub struct SyntaxMatcher<'a> {
    //Used to lookup for variable definitions and so on
    syntax: Rc<RefCell<Vec<SyntaxItem<'a>>>>,
    matchers: Vec<BreadcrumbsMatcher<'a>>,
    toml: &'a Table,
    colors: HashMap<&'a str, &'a str>,
}

impl<'a> SyntaxMatcher<'a> {
    pub fn from_toml(toml: &'a Table) -> anyhow::Result<Self> {
        let syntax = Rc::new(RefCell::new(vec![]));

        let mut defined_groups: HashSet<&str> = HashSet::new();
        let mut used_groups: HashSet<&str> = HashSet::new();

        let mut groups: Vec<(Vec<Vec<MatchPattern>>, &str, SyntaxItemType)> = vec![];
        let mut colors = HashMap::new();

        for (name, content) in toml.iter() {
            //Anything starting with 'denshi' is a highlight group
            if name.starts_with("colors") {
                if let toml::Value::Table(ref table_inner) = content {
                    for (group_name, color_str) in table_inner {
                        if let toml::Value::String(inner_str) = color_str {
                            colors.insert(group_name.as_ref(), inner_str.as_ref());
                        } else {
                            bail!("Found {color_str} in \"{group_name}\" of \"colors\" table")
                        }
                    }
                }
                continue;
            }
            match content {
                toml::Value::Table(ref table_inner) => {
                    let mut filters = vec![];
                    if let Some(toml::Value::Array(pattern_list)) = table_inner.get("patterns") {
                        if pattern_list.len() == 0 {
                            bail!("Length of \'patterns\' in {name} can't be 0");
                        }
                        for k in pattern_list.iter() {
                            if let toml::Value::String(pattern) = k {
                                let mut filter_match = vec![];
                                for k in pattern.split(" ").into_iter() {
                                    let first_char = &k[0..1]; //.expect("Syntax identifier should be longer than 0");
                                    if first_char == "^" {
                                        filter_match.push(MatchPattern::NotMatches(k[1..].into()));
                                    } else {
                                        filter_match.push(MatchPattern::Matches(k.into()));
                                    }
                                }
                                if filter_match.is_empty() {
                                    bail!("Match pattern is empty for group {name}");
                                }
                                filters.push(filter_match);
                            } else {
                                bail!("Found {k:?} in pattern {name}");
                            }
                        }
                    } else {
                        bail!("\'patterns\' array not found in group {name}");
                    }
                    let syntax_type = match table_inner.get("ifDefined") {
                        Some(toml::Value::String(pattern)) => {
                            used_groups.insert(pattern);
                            if let Some(toml::Value::String(other_group)) =
                                table_inner.get("orElse")
                            {
                                SyntaxItemType::IfDefinedElse(pattern, other_group)
                            } else {
                                SyntaxItemType::IfDefined(pattern)
                            }
                        }
                        _ => SyntaxItemType::Always,
                    };

                    defined_groups.insert(name);

                    let filter_group = (filters, name.as_ref(), syntax_type);
                    groups.push(filter_group);
                }
                other => bail!("Found {other:?} in toml"),
            }
        }

        for used_group in used_groups.iter() {
            if !defined_groups.contains(used_group) {
                bail!("Group used in IfDefined \"{used_group}\" does not exist");
            }
        }

        let mut matchers = vec![];
        for group in groups {
            let inner_vec = Rc::clone(&syntax);
            let callback = move |locate: &sv_parser::Locate| {
                inner_vec.borrow_mut().push(SyntaxItem {
                    group: group.1,
                    col_start: locate.offset,
                    col_end: locate.offset + locate.len,
                    syntax_type: group.2,
                });
            };

            for pattern in group.0 {
                let bc = BreadcrumbsMatcher::new(pattern, Box::new(callback.clone()));
                matchers.push(bc)
            }
        }
        Ok(Self {
            toml,
            matchers,
            syntax,
            colors,
        })
    }

    pub fn get_colors(&self) -> HashMap<&str, &str> {
        return self.colors.clone();
    }

    pub fn get_colors_as_ansi(&self) -> anyhow::Result<HashMap<String, String>> {
        let mut output = HashMap::new();

        for (group, color_str) in &self.colors {
            let mut codes = vec![];

            for command in color_str.split(" ") {
                let mut split = command.split("=");
                let cmd = split.next().context(format!(
                    "Expected group {group}, command {command} to contain an '='"
                ))?;
                let value = split.next().context(format!(
                    "Expected group {group}, command {command} to contain an '='"
                ))?;
                match cmd {
                    "ctermfg" => {
                        codes.push("38".to_string());
                        codes.push("5".to_string());
                        codes.push(value.to_string());
                    }
                    "cterm" => codes.push(
                        value
                            .split(",")
                            .map(|cterm_code| {
                                Ok(match cterm_code {
                                    "bold" => "1",
                                    "italic" => "3",
                                    "underline" => "4",
                                    patt => bail!("Unknown pattern {patt}"),
                                })
                            })
                            .collect::<Result<Vec<_>, _>>()?
                            .join(";"),
                    ),
                    "guifg" => (),
                    patt => bail!("Unknown command {patt}"),
                }
            }
            output.insert((*group).to_string(), format!("\x1b[{}m", codes.join(";")));
        }

        Ok(output)
    }

    pub fn enter(&mut self, node: &RefNode) {
        for m in &mut self.matchers {
            m.enter(node);
        }
    }

    pub fn leave(&mut self, node: &RefNode) {
        for m in &mut self.matchers {
            m.leave(node);
        }
    }

    pub fn compute(self, code: &'a str) -> Vec<MatchedSyntaxItem<'a>> {
        let mut requiring_defs = vec![];
        let mut output_str: Vec<MatchedSyntaxItem> = vec![];

        //Need to drop matchers here, as this drops all the closures, which have refs to the Rc
        drop(self.matchers);

        let current_list = Rc::try_unwrap(self.syntax)
            .expect("Should have no references to this RC now")
            .into_inner()
            .into_iter()
            .unique();

        let mut keyword_map: HashMap<&str, HashSet<&str>> = HashMap::new();

        for item in current_list {
            let matched = &code[item.col_start..item.col_end];
            if let SyntaxItemType::Always = item.syntax_type {
                keyword_map
                    .entry(item.group)
                    .or_insert(HashSet::new())
                    .insert(matched);
                output_str.append(&mut MatchedSyntaxItem::from_unmatched(item, code));
            } else {
                requiring_defs.push(item);
            }
        }

        for mut item in requiring_defs {
            let matched_str = &code[item.col_start..item.col_end];
            match item.syntax_type {
                SyntaxItemType::IfDefined(predicate_group) => {
                    if keyword_map
                        .get(predicate_group)
                        .map_or(false, |x| x.contains(matched_str))
                    {
                        output_str.append(&mut MatchedSyntaxItem::from_unmatched(item, code))
                    }
                }
                SyntaxItemType::IfDefinedElse(predicate_group, other_group) => {
                    if !keyword_map
                        .get(predicate_group)
                        .map_or(false, |x| x.contains(matched_str))
                    {
                        item.group = other_group
                    }

                    output_str.append(&mut MatchedSyntaxItem::from_unmatched(item, code))
                }
                SyntaxItemType::Always => unreachable!(),
            }
        }

        //Does all the println b
        /*
        println!(
            "{} {line} {col_start} {col_end} {}",
            cl_group, matched_chars
        );

        */
        return output_str;
    }
}
use itertools::Itertools;
