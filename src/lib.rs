
extern crate proc_macro;

use std::{fs, process::Command, str::FromStr};

use itertools::Itertools;
use map_tuple::*;
use proc_macro::{Delimiter, TokenStream, TokenTree};
use proc_macro_error::{emit_call_site_error, proc_macro_error};


#[allow(unstable_name_collisions)]
#[proc_macro_error(proc_macro_hack)]
#[proc_macro_attribute]
pub fn unpack(key: TokenStream, item: TokenStream) -> TokenStream {
    let key_string = key.to_string();
    let debug = key_string.contains("debug");
    let silent = key_string.contains("no_info");

    let mut header_text = "".to_string();
    let mut function_name = "".to_string();
    let mut parameters = "".to_string();
    let mut body_text = "".to_string();

    let mut last_fn_token = false;
    let mut header_done = false;
    for token in item.clone() {
        if !header_done {
            if let TokenTree::Group(group ) = &token {
                if Delimiter::Parenthesis == group.delimiter() {
                    header_done = true; 
                    
                    parameters = group.stream()
                        .into_iter()
                        .map(|token| token.to_string())
                        .intersperse(" ".to_string())
                        .collect();
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
            let spacing = match &token {
                TokenTree::Group(_) => " ",
                TokenTree::Ident(_) =>  " ",
                TokenTree::Punct(_) => "",
                TokenTree::Literal(_) => " ",
            };

            body_text.push_str(&token.to_string());
            body_text.push_str(spacing);
        }
    }
 
    if parameters.is_empty() {
        emit_call_site_error!("Function has no parameter!");
    }

    let mut paramter_split = parameters.split(",");
    let self_text = paramter_split.next()
        .expect("Parameter are empty!")
        .to_string();

    if self_text != "& mut self " && self_text != "& self " && self_text != "self " && self_text == "mut self " {
        emit_call_site_error!("First parameter is not self!");
    }
    let pre_self = self_text.replace("self", "")
        .chars()
        .filter(|c| *c != ' ')
        .collect::<String>();

    let parameter_text = paramter_split
        .intersperse(",")
        .collect::<String>(); 

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
            line.split(":").tuples()
        })
        .flatten()
        .map(|(key, typ)| {
            let key = key.chars()
                .filter(|c| *c != ' ')
                .collect::<String>();

            let key = key.replace("pub", "");

            let typ = typ.split(',')
                .filter(|s| !s.chars()
                    .all(|c| c == ' '))
                .intersperse(",")
                .collect::<String>();
            
            (key, typ)
        })
        .collect::<Vec<_>>();
    if debug {
        dbg!(&lines);
    }

    let body_text_ref = &body_text; 
    let (new_body, (unpacking_usefull, new_fields)) = body_text.match_indices("self.")
        .map(|(i, _)| i + 5)
        .map(|start| {
            let (end, char) = body_text[start..].match_indices([' ', '\n', '.', ';', ',', '(', ')', '[', ']', '{', '}',  '=' ])
                .next()
                .expect("self. did not end!");
            let end = end + start;

            if char == "(" {
                panic!("{function_name} contains a function call on self!")
            }

             
            (start, end)
        })
        .tee()
        .map0(|x|x 
            .map(|(start, end)| {
                let mut add_deref = false;

                // special with &mut self case self.a = x -> *a = x
                if pre_self == "&mut" {
                    let next_punct = body_text[start..].match_indices(['.', ';', '(', ')', '[', ']', '{', '}', '=' ])
                        .map(|(_, c)| c)
                        .next()
                        .expect("self.<Name> did not follow with a puct!");
                    if next_punct == "=" {
                        add_deref = true;
                    }
                }

                (start, end, add_deref)
            })
            .fold(("".to_string(), 0), |(mut text, last), (start, end, add_deref)| {
                let mut body_end = start - 5;
                body_end -= body_text[last..body_end].chars()
                    .rev()
                    .take_while(|c| *c == ' ')
                    .count();

                if body_end >= 3 && &body_text[(body_end - 3)..body_end] == "mut" {
                    body_end -= 3;
                }

                body_end -= body_text[last..body_end].chars()
                    .rev()
                    .take_while(|c| *c == ' ')
                    .count();

                if body_end >= 1 && &body_text[(body_end - 1)..body_end] == "&" {
                    body_end -= 1;
                }

                text.push_str(&body_text[last..body_end]);

                if add_deref {
                    text.push_str("*(");
                    text.push_str(&body_text[start..end]);
                    text.push_str(")");
                } else {
                    text.push_str(&body_text[start..end]);
                } 
                
                (text, end)
            })
        )
        .map0(|(mut text, end)|{
            text.push_str(&body_text[end..]);
            text
        })
        .map1(|x|x
            .fold(lines.iter()
                .map(|a| (false, a))
                .collect::<Vec<_>>(), 
                |mut lines, (start, end)| {
                for (b, (key, _)) in lines.iter_mut() {
                    *b |= *key == &body_text_ref[start..end];
                }

                lines
            }).into_iter()
            .tee()
            .map0(|mut x| !x.all(|(b, _)| b))
            .map1(|x|x
                .filter(|(b, _)| *b)
                .fold("".to_string(), |parameters, (_, (key, typ))| {
                    if !typ.contains("&") {
                        format!("{key}: {pre_self} {typ}, {parameters}")
                    } else {
                        format!("{key}: {typ}, {parameters}")
                    }
            }))); 

    let final_text = format!("{}\n\n{header_text}({new_fields}{parameter_text})\n{new_body}", item.to_string());
    
    if debug {
        println!("   > Unpacked Debug: \n {final_text}\n");
        
        if !unpacking_usefull {
            println!("[WARN] All fields are used in function. Unpacking does not help.")
        }

    } else if !silent {
        println!("   > Unpacked {header_text}({new_fields}...)");

        if !unpacking_usefull {
            println!("[WARN] All fields are used in function. Unpacking does not help.")
        }
    }
    
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
