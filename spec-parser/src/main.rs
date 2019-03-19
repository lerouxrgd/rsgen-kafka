mod templates;

use failure::Error;
use heck::CamelCase;
use lazy_static::*;
use pest::Parser;
use pest_derive::*;
use regex::Regex;
use templates::Templater;

// TODO: move all this in a dedicated `parser.rs` module

#[derive(Parser)]
#[grammar = "protocol.pest"]
pub struct ProtocolParser;

fn capped_comment(text: &str, nb_indent: usize) -> String {
    lazy_static! {
        static ref RE: Regex = Regex::new(r"\b.{1,55}\b\W?").expect("Invalid regex");
    }
    let comment = if nb_indent > 0 {
        format!("{}///", " ".repeat(nb_indent))
    } else {
        String::from("///")
    };
    RE.captures_iter(text)
        .into_iter()
        .filter_map(|c| c.get(0))
        .map(|c| format!("{} {}", comment, c.as_str()))
        .collect::<Vec<_>>()
        .as_slice()
        .join("\n")
}

fn wip_parsing() -> Result<(), Error> {
    // wget https://kafka.apache.org/21/protocol.html
    let raw = include_str!("protocol.html");

    let templater = Templater::new()?;

    let file = ProtocolParser::parse(Rule::file, &raw)
        .expect("Unsuccessful parsing")
        .next() // there is exactly one { file }
        .unwrap();

    let mut skip_req_resp = 19;
    for target in file.into_inner() {
        match target.as_rule() {
            Rule::error_codes => {
                let err_code_rows = target
                    .into_inner() // inner { table }
                    .next() // there is exactly one { table }
                    .unwrap()
                    .into_inner() // inner { tr }
                    .into_iter()
                    .map(|tr| {
                        let row = tr
                            .into_inner() // inner { td }
                            .into_iter()
                            .map(|td| td.into_inner().as_str()) // inner { content }
                            .collect::<Vec<_>>();
                        (
                            String::from(row[0]).to_camel_case(),
                            String::from(row[1]),
                            capped_comment(&format!("{} Retriable: {}.", row[3], row[2]), 4),
                        )
                    })
                    .collect::<Vec<_>>();
                let s = templater.str_err_codes(&err_code_rows);
                // println!("{}", s.unwrap());
            }

            Rule::api_keys => {
                let api_key_rows = target
                    .into_inner() // inner { table }
                    .next() // there is exactly one { table }
                    .unwrap()
                    .into_inner() // inner { tr }
                    .into_iter()
                    .map(|tr| {
                        let row = tr
                            .into_inner() // inner { td }
                            .into_iter()
                            .map(|td| {
                                td.into_inner() // inner { a }
                                    .next() // there is exactly one { a }
                                    .unwrap()
                                    .into_inner() // inner { content }
                                    .as_str()
                            })
                            .collect::<Vec<_>>();
                        (String::from(row[0]), String::from(row[1]))
                    })
                    .collect::<Vec<_>>();
                let s = templater.str_api_keys(&api_key_rows);
                // println!("{}", s.unwrap());
            }

            Rule::req_resp => {
                if skip_req_resp > 0 {
                    skip_req_resp -= 1;
                    continue;
                }

                for section in target.into_inner() {
                    match section.as_rule() {
                        Rule::table => {
                            let param_rows = section
                                .into_inner() // inner { td }
                                .map(|tr| {
                                    let row = tr
                                        .into_inner() // inner { td }
                                        .into_iter()
                                        .map(|td| td.into_inner().as_str()) // inner { content }
                                        .collect::<Vec<_>>();
                                    (String::from(row[0]), String::from(row[1]))
                                })
                                .collect::<Vec<_>>();
                            // println!("{:?}", param_rows);
                        }

                        Rule::content => {
                            println!("{}", section.as_str());
                            wip_bnf(section.as_str());
                            // println!("====> {:?}", section.as_str());
                        }

                        _ => unreachable!(),
                    }
                }

                break;
            }

            _ => (),
        }
    }

    Ok(())
}

fn type_for(name: &str) -> String {
    let name = if name.chars().nth(0).expect("no first char") == '['
        && name.chars().last().expect("no last char") == ']'
    {
        &name[1..name.len() - 1]
    } else {
        name
    };
    String::from(name).to_camel_case()
}

#[derive(Debug)]
enum SpecVal<'a> {
    Primitive(&'a str),
    Struct(Vec<(&'a str, SpecVal<'a>)>),
    Array(Vec<SpecVal<'a>>),
}

fn wip_bnf(raw: &str) {
    let raw = "CreateTopics Request (Version: 0) => [create_topic_requests] timeout \n  create_topic_requests => topic num_partitions replication_factor [replica_assignment] [config_entries] \n    topic => STRING\n    num_partitions => INT32\n    replication_factor => INT16\n    replica_assignment => partition [replicas] \n      partition => INT32\n      replicas => INT32\n    config_entries => config_name config_value \n      config_name => STRING\n      config_value => NULLABLE_STRING\n  timeout => INT32";

    println!("{}", raw);
    let yo = raw.split('\n').collect::<Vec<_>>();
    let (first, rest) = yo.split_first().unwrap();

    lazy_static! {
        static ref RE: Regex =
            Regex::new(r"(\w+) (\w+) (\(Version: (\d+)\) )?=>(.*)").expect("Invalid regex");
    }
    let caps = RE.captures(first);
    println!("{:?}", caps);

    #[derive(Debug)]
    enum Type<'a> {
        Primitive(&'a str),
        Struct(Vec<&'a str>),
    }

    #[derive(Debug)]
    struct Line<'a> {
        indent: usize,
        ret: Type<'a>,
    }

    struct Acc<'a> {
        spec: Option<SpecVal<'a>>,
        buffer: Vec<Line<'a>>,
        input: Vec<Line<'a>>,
    }

    fn yoyo(mut acc: Acc) -> Acc {
        match acc {
            Acc { spec: None, .. } => {
                acc.spec = Some(SpecVal::Primitive("INT8"));
                acc
            }
            _ => {
                acc.spec = Some(SpecVal::Primitive("INT16"));
                acc
            }
        }
    };

    let mut spec = SpecVal::Struct(vec![]);

    let input = rest
        .to_vec()
        .into_iter()
        .map(|s| {
            let parts = s.split(" =>").collect::<Vec<_>>();

            let indent = parts.get(0).unwrap().matches(' ').count();

            let ret = parts
                .get(1)
                .unwrap()
                .split(' ')
                .filter(|p| *p != "")
                .collect::<Vec<_>>();
            let ret = if ret.len() == 1 {
                Type::Primitive(ret[0])
            } else {
                Type::Struct(ret)
            };

            Line { indent, ret }
        })
        .collect::<Vec<_>>();

    let mut acc = Acc {
        spec: None,
        buffer: vec![],
        input,
    };

    // TODO: generate root from `first` line
    let root = vec!["[create_topic_requests]", "timeout"];
    for field in root {
        if let SpecVal::Struct(ref mut fields) = spec {
            acc = yoyo(acc);
            let spec = acc.spec.take().unwrap();
            fields.push((field, spec));
        }
    }

    acc.input.into_iter().for_each(|x| println!("{:?}", x));
    println!("{:?}", spec);
}

fn main() {
    // wip_parsing().unwrap();
    wip_bnf("");
}
