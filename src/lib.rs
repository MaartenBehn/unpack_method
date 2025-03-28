
extern crate proc_macro;

use std::{fs, process::Command, str::FromStr};

use itertools::Itertools;
use proc_macro::{Delimiter, TokenStream, TokenTree};
use proc_macro_error::{emit_call_site_error, proc_macro_error};

#[proc_macro_error(proc_macro_hack)]
#[proc_macro_attribute]
pub fn unpack(_: TokenStream, item: TokenStream) -> TokenStream {
    let mut header_text = "".to_string();
    let mut function_name = "".to_string();
    let mut parameters = vec![];
    let mut body_text = "".to_string(); 

    let mut last_fn_token = false;
    let mut header_done = false;
    for token in item.clone() {
        if !header_done {
            if let TokenTree::Group(group ) = &token {
                if Delimiter::Parenthesis == group.delimiter() {
                    header_done = true; 
                    
                    let mut parameter = vec![];
                    for token in group.stream() {
                        let mut flush = false;
                        if let TokenTree::Punct(punct) = &token {
                            if punct.as_char() == ',' {
                                flush = true;
                            }
                        }

                        if flush {
                            parameters.push(parameter);
                            parameter = vec![];
                        } else {
                            parameter.push(token);
                        }
                    }
                    parameters.push(parameter);
                }
            }

            if !header_done {
                if last_fn_token {
                    if let TokenTree::Ident(ident) = &token {
                        function_name = ident.to_string();
                        let new_function_name = format!("{function_name}_unpacked");
                        header_text = format!("{header_text} {new_function_name}");
                    }
                } else {
                    header_text = format!("{header_text} {}", token.to_string());
                }
            } 

            if let TokenTree::Ident(ident) = &token {
                if ident.to_string() == "fn" {
                    last_fn_token = true;
                }
            }
        } else {
            body_text = format!("{body_text}{}", token.to_string());
        }
    }
 
    if parameters.is_empty() {
        emit_call_site_error!("Function has no parameter!");
    }

    let text = parameters[0].iter() 
        .map(|token| format!("{} ", token.to_string()))
        .collect::<String>();
    if text != "& mut self " && text != "& self " && text != "self " && text == "mut self " {
        emit_call_site_error!("First parameter is not self!");
    }
    let pre_self = text.replace("self", "")
        .chars()
        .filter(|c| *c != ' ')
        .collect::<String>();

    let parameter_texts = parameters[1..].iter()
        .map(|token_tree| {
            token_tree.iter()
                .map(|token | format!("{} ", token.to_string()))
                .collect::<String>()
        })
        .collect::<Vec<_>>();
 


    let function_file_path = find_function(&function_name);
    let content = fs::read_to_string(&function_file_path).expect("Function file not found!");

    let function_indecies = content.match_indices(&format!("fn {function_name}"))
        .map(|(i, _)|i)
        .collect::<Vec<_>>();

    if function_indecies.len() != 1 {
        emit_call_site_error!("\"fn {}\" should be unique in file {}", function_name, function_file_path);
    }

    let function_index = function_indecies[0];

    let impl_start = content[..function_index].match_indices("impl")
        .map(|(i, _)|i)
        .last()
        .expect("function needs to be in an impl Block");

    let block_open = content[impl_start..function_index].match_indices("{")
        .map(|(i, _)|i)
        .next()
        .expect("no \"{\" between impl and function") + impl_start;

    let impl_section = content[(impl_start + 4)..block_open]
        .chars()
        .filter(|c| *c != ' ')
        .collect::<String>();

    let generic_blocks = impl_section.match_indices(['<', '>'])
        .tuples()
        .map(|((i, a), (j, b))| {
            if a == "<" && b == ">" {
                (i, j)
            } else {
                panic!("{} has invalid ammount of \"<\" and \">\"", impl_section);
            }
        }).collect::<Vec<_>>();

    let struct_name = generic_blocks.into_iter()
        .rev()
        .fold(impl_section, |impl_section, (i, j)| {
            impl_section.chars()
                .enumerate()
                .filter(|(a, _)| *a < i || j < *a )
                .map(|(_, a)| a)
                .collect()
        });

    let struct_file = find_struct(&struct_name);
    let content = fs::read_to_string(&struct_file).expect(&format!("Struct file at {struct_file} not found!"));

    let struct_index = content.match_indices(&format!("struct {struct_name}"))
        .map(|(i, _)|i)
        .next()
        .expect("Did not find struct in struct file");

    let struct_end = content[struct_index..].match_indices("}")
        .map(|(i, _)|i)
        .next()
        .expect("Did not find struct end") + struct_index;

    let struct_start = content[struct_index..struct_end].match_indices("{")
        .map(|(i, _)|i)
        .next()
        .expect("Did not find struct start") + struct_index + 1;

    let lines = content[struct_start..struct_end].lines()
        .map(|line| {
            line.chars()
                .filter(|c| *c != ' ' && *c != ',')
                .chunk_by(|c| *c == ':')
                .into_iter()
                .map(|(_, c)| c.collect::<String>())
                .filter(|s| s != ":")
                .tuples::<(String, String)>()
                .next()
        })
        .flatten()
        .collect::<Vec<_>>();

    
    let new_fields = body_text.match_indices("self.")
        .map(|(i, _)| i + 5)
        .map(|start| {
            let (end, char) = body_text[start..].match_indices(['.', '(', ')', ' ', '[', ']', '\n', ';' ])
                .next()
                .expect("self. did not end!");

            if char == "(" {
                panic!("{function_name} contains a function call on self!")
            }

            (start, end + start)
        }).fold(lines.into_iter()
            .map(|a| (false, a))
            .collect::<Vec<_>>(), 
            |mut lines, (start, end)| {
            let field = &body_text[start..end];
            for (b, (key, _)) in lines.iter_mut() {
                *b |= *key == field;
            }

            lines
        }).into_iter()
        .filter(|(b, _)| *b)
        .fold("".to_string(), |a, (_, (key, typ))| format!("{key}: {pre_self} {typ}, {a}"));

    let body_text = body_text.replace("self.", ""); 
    
    let parameter_text = parameter_texts.into_iter()
        .map(|text| format!(" {text}"))
        .collect::<String>();

    println!("   > Unpacked: {header_text}({new_fields}...)");

    let final_text = format!("{}\n \n \n {header_text}({new_fields}{parameter_text}){body_text}", item.to_string());
    proc_macro::TokenStream::from_str(&final_text).unwrap()
}

fn find_function(name: &str) -> String {
    let bytes = Command::new("grep")
        .arg("--include=*.rs")
        .arg("-rw")
        .arg(".")
        .arg("-e")
        .arg(format!("fn {name}"))
        .output()
        .expect("grep command failed to start")
        .stdout;

    let line = String::from_utf8(bytes).expect("grep output should be valid utf8");
    // (&line);

    let mut split = line.split(":");
    let path = split.next().expect("grep output is empty!");

    path.to_owned()
}

fn find_struct(name: &str) -> String {
    let bytes = Command::new("grep")
        .arg("--include=*.rs")
        .arg("-rw")
        .arg(".")
        .arg("-e")
        .arg(format!("struct {name}"))
        .output()
        .expect("grep command failed to start")
        .stdout;

    let line = String::from_utf8(bytes).expect("grep output should be valid utf8");

    let mut split = line.split(":");
    let path = split.next().expect("grep output is empty!");

    path.to_owned()
}
