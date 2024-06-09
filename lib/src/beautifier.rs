/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/.
 */

use std::io::Write;
use std::process::{Command, Stdio};

use super::args::Arguments;
use anyhow::{anyhow, Context, Result};
use tree_sitter::Node;

struct State<'a> {
    formatted: String,
    arguments: &'a mut Arguments,
    code: &'a [u8],
    col: usize,
    row: usize,
    level: usize,
    extra_indentation: usize,
    num_spaces: usize,
}

impl State<'_> {
    fn indent(&mut self) {
        for _ in 0..self.level {
            self.print(" ".repeat(self.num_spaces).as_str());
        }
        for _ in 0..self.extra_indentation {
            self.print(" ");
        }
    }

    fn print(&mut self, string: &str) {
        if self.arguments.inplace {
            self.formatted += string;
        } else {
            print!("{}", string);
        }
        self.col += string.len();
    }

    fn print_node(&mut self, node: Node) -> Result<()> {
        self.print(node.utf8_text(self.code)?);
        Ok(())
    }

    fn println(&mut self, string: &str) {
        if self.arguments.inplace {
            self.formatted += string;
            self.formatted += "\n";
        } else {
            println!("{}", string);
        }
        self.col = 0;
        self.row += 1;
    }
}

trait TraversingError<T> {
    fn err_at_loc(self, node: &Node) -> Result<T>;
}

impl<T> TraversingError<T> for Option<T> {
    fn err_at_loc(self, node: &Node) -> Result<T> {
        self.ok_or_else(|| {
            anyhow!(
                "Error accessing token around line {} col {}",
                node.range().start_point.row,
                node.range().start_point.column
            )
        })
    }
}

pub fn beautify(code: &str, arguments: &mut Arguments) -> Result<String> {
    let mut parser = tree_sitter::Parser::new();
    parser
        .set_language(&tree_sitter_wbproto::language())
        .with_context(|| "Could not set Tree-Sitter language")?;

    let tree = parser
        .parse(code, None)
        .ok_or_else(|| anyhow!("Could not parse file."))?;

    let root = tree.root_node();
    if root.has_error() {
        return Err(anyhow!("Parsed file contain errors."));
    }

    let mut state = State {
        arguments,
        code: code.as_bytes(),
        col: 0,
        row: 0,
        level: 0,
        extra_indentation: 0,
        formatted: String::with_capacity(code.len() * 2),
        num_spaces: 4,
    };

    format_document(&mut state, root)?;
    Ok(state.formatted)
}

fn format_document(state: &mut State, node: Node) -> Result<()> {
    let mut cursor = node.walk();
    let children: Vec<Node> = node.children(&mut cursor).collect();
    for child in children {
        if format_node(state, child).is_err() {
            eprintln!("Failed to format node.");
            break;
        }
        state.println("");
    }
    Ok(())
}

fn format_node(state: &mut State, node: Node) -> Result<()> {
    match node.kind() {
        "class" => format_class(state, node),
        "comment" => format_comment(state, node),
        "extern" => format_extern(state, node),
        "property" => format_property(state, node),
        "proto" => format_proto(state, node),
        "vector" => format_vector(state, node),
        "javascript" => format_javascript(state, node),
        _ => state.print_node(node),
    }
}

fn format_comment(state: &mut State, node: Node) -> Result<()> {
    let text = node.utf8_text(state.code)?;
    let line = text.strip_prefix('#').unwrap_or(text).trim();
    state.print("#");
    if !line.starts_with("VRML") {
        state.print(" ");
    }
    state.print(line);
    Ok(())
}

fn format_extern(state: &mut State, node: Node) -> Result<()> {
    let mut cursor = node.walk();
    let children: Vec<Node> = node.named_children(&mut cursor).collect();
    let text = children.first().err_at_loc(&node)?.utf8_text(state.code)?;
    state.print("EXTERNPROTO ");
    state.print(text);
    Ok(())
}

fn format_proto(state: &mut State, node: Node) -> Result<()> {
    let name = node.child_by_field_name("proto").err_at_loc(&node)?;
    let mut cursor = node.walk();
    let fields: Vec<Node> = node
        .named_children(&mut cursor)
        .filter(|n| n.kind() == "field")
        .collect();
    let sizes = field_sizes(state, fields);

    state.println("");
    state.print("PROTO ");
    state.print(name.utf8_text(state.code)?);
    state.print(" [");
    state.level = 1;
    let mut last_line = 0;
    let mut ok = false;
    for child in node.children(&mut cursor) {
        match (child.kind(), ok) {
            ("[", false) => ok = true,
            ("]", true) => ok = false,
            ("field", true) => {
                state.println("");
                state.indent();
                last_line = child.range().end_point.row;
                let mut at = state.level * state.num_spaces + sizes.0;
                let mut ccursor = node.walk();
                let fields: Vec<Node> = child.children(&mut ccursor).collect();
                format_node(
                    state,
                    *fields
                        .first()
                        .ok_or_else(|| anyhow!("Could not extract field kind"))?,
                )?;
                state.print(" ".repeat(at.saturating_sub(state.col)).as_str());
                format_node(
                    state,
                    *fields
                        .get(1)
                        .ok_or_else(|| anyhow!("Could not extract field type"))?,
                )?;
                at += sizes.1;
                state.print(" ".repeat(at.saturating_sub(state.col)).as_str());
                format_node(
                    state,
                    *fields
                        .get(2)
                        .ok_or_else(|| anyhow!("Could not extract field name"))?,
                )?;
                at += sizes.2;
                state.print(" ".repeat(at.saturating_sub(state.col)).as_str());
                format_node(
                    state,
                    *fields
                        .get(3)
                        .ok_or_else(|| anyhow!("Could not extract field value"))?,
                )?;
            }
            ("comment", true) => {
                if child.range().start_point.row != last_line {
                    state.println("");
                    state.indent();
                } else {
                    let at = state.level * state.num_spaces + sizes.0 + sizes.1 + sizes.2 + sizes.3;
                    state.print(" ".repeat(at.saturating_sub(state.col)).as_str());
                }
                format_comment(state, child)?;
                last_line = state.row;
            }
            (_, _) => continue,
        }
    }
    state.println("");
    state.println("]");
    state.println("{");
    state.level = 0;
    let mut ok = false;
    for child in node.children(&mut cursor) {
        match (child.kind(), ok) {
            ("{", false) => {
                ok = true;
                continue;
            }
            ("class", true) => format_class(state, child)?,
            ("comment", true) => format_comment(state, child)?,
            ("javascript", true) => format_node(state, child)?,
            (_, _) => continue,
        }
        state.println("");
    }
    state.println("}");
    Ok(())
}

fn field_sizes(state: &mut State, fields: Vec<Node>) -> (usize, usize, usize, usize) {
    let mut kind_size = 0usize;
    let mut type_size = 0usize;
    let mut name_size = 0usize;
    let mut value_size = 0usize;

    let saved_formatted = state.formatted.clone();
    let saved_inplace = state.arguments.inplace;
    state.formatted.clear();
    state.arguments.inplace = true;

    for field in fields {
        let mut cursor = field.walk();
        let children: Vec<Node> = field.children(&mut cursor).collect();

        let node_kind = children.first().unwrap();
        format_node(state, *node_kind).unwrap();
        let text_kind = state.formatted.clone();
        state.formatted.clear();

        let node_type = children.get(1).unwrap();
        format_node(state, *node_type).unwrap();
        let text_type = state.formatted.clone();
        state.formatted.clear();

        let node_name = children.get(2).unwrap();
        format_node(state, *node_name).unwrap();
        let text_name = state.formatted.clone();
        state.formatted.clear();

        let node_value = children.get(3).unwrap();
        format_node(state, *node_value).unwrap();
        let text_value = state.formatted.clone();
        state.formatted.clear();

        let padding = state.num_spaces;
        kind_size = std::cmp::max(kind_size, text_kind.len() + padding);
        type_size = std::cmp::max(type_size, text_type.len() + padding);
        name_size = std::cmp::max(name_size, text_name.len() + padding);
        value_size = std::cmp::max(value_size, text_value.len() + padding);
    }

    state.formatted = saved_formatted;
    state.arguments.inplace = saved_inplace;

    (kind_size, type_size, name_size, value_size)
}

fn format_class(state: &mut State, node: Node) -> Result<()> {
    let identifier = node.child(0).err_at_loc(&node)?.utf8_text(state.code)?;
    state.print(identifier);
    state.print(" {");

    let mut ok = false;
    let mut cursor = node.walk();
    let mut last_row = 0;

    state.level += 1;
    for child in node.children(&mut cursor) {
        match (child.kind(), ok) {
            ("{", false) => ok = true,
            ("}", true) => ok = false,
            ("comment", true) => {
                if last_row != child.range().start_point.row {
                    state.println("");
                    state.indent();
                }
                format_comment(state, child)?;
            }
            (_, true) => {
                state.println("");
                state.indent();
                format_node(state, child)?;
            }
            (_, false) => continue,
        }
        last_row = child.range().end_point.row;
    }
    state.level -= 1;

    state.println("");
    state.indent();
    state.print("}");
    Ok(())
}

fn format_property(state: &mut State, node: Node) -> Result<()> {
    let mut first = true;
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if !first {
            state.print(" ");
        }
        first = false;
        format_node(state, child)?;
    }
    Ok(())
}

fn format_vector(state: &mut State, node: Node) -> Result<()> {
    let mut oneliner = node.range().start_point.row == node.range().end_point.row;
    let mut cursor = node.walk();
    let contains_class = node.children(&mut cursor).any(|n| n.kind() == "class");
    if contains_class {
        oneliner = false;
    }
    let mut last_node = node;
    for child in node.children(&mut cursor) {
        match child.kind() {
            "[" => {
                if oneliner {
                    state.print("[");
                } else {
                    state.print("[");
                    state.level += 1;
                    state.println("");
                    state.indent();
                }
                last_node = node;
                continue;
            }
            "]" => {
                if oneliner {
                    state.print("]");
                } else {
                    state.level -= 1;
                    state.println("");
                    state.indent();
                    state.print("]");
                }
            }
            "," => {
                state.print(",");
                if !oneliner {
                    last_node = node;
                    state.println("");
                    state.indent();
                    continue;
                }
            }
            "comment" => {
                if last_node != node
                    && (last_node.kind() == "comment"
                        || last_node.range().end_point.row != child.range().start_point.row)
                {
                    state.println("");
                    state.indent();
                }
                state.print(" ");
                format_comment(state, child)?;
            }
            _ => {
                if oneliner {
                    // First node in row
                    if last_node != node {
                        state.print(" ");
                    }
                } else {
                    if last_node.kind() == "class" {
                        state.println("");
                        state.indent();
                    }
                    // First node in row
                    if last_node != node && last_node.kind() != "class" {
                        state.print(" ");
                    }
                }
                format_node(state, child)?;
            }
        }
        last_node = child;
    }
    Ok(())
}

fn format_javascript(state: &mut State, node: Node) -> Result<()> {
    let oneliner = node.range().start_point.row == node.range().end_point.row;
    let opener = node.child(0).err_at_loc(&node)?.utf8_text(state.code)?;
    let mut cursor = node.walk();

    let code = node
        .children(&mut cursor)
        .find(|n| n.kind() == "code")
        .err_at_loc(&node)?
        .utf8_text(state.code)?;
    let mut clang_format = Command::new("clang-format")
        .arg("-assume-filename")
        .arg("code.js")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .expect("clang-format command failed to start");

    if let Some(mut stdin) = clang_format.stdin.take() {
        stdin
            .write_all(code.as_bytes())
            .expect("Failed to write to stdin");
    } else {
        panic!("Failed to open stdin");
    }

    let raw_output = clang_format
        .wait_with_output()
        .expect("Failed to read stdout");

    let formatted_code = String::from_utf8(raw_output.stdout).expect("Output is not valid UTF-8");
    let formatted_code = formatted_code.trim();

    if oneliner {
        state.print(opener);
        state.print(" ");
        state.print(formatted_code);
        state.print(" >%");
    } else {
        state.indent();
        state.println(opener);
        state.level += 1;
        for line in formatted_code.lines() {
            state.indent();
            state.println(line);
        }
        state.level -= 1;
        state.indent();
        state.println(">%");
    }
    Ok(())
}
