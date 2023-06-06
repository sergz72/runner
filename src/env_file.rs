use std::collections::HashMap;
use std::fs::File;
use std::io::{BufRead, BufReader, Error, ErrorKind};

pub fn parse_env_file(file_name: String) -> Result<HashMap<String, String>, Error> {
    let file = File::open(&file_name)?;
    let lines = BufReader::new(file).lines()
        .map(|l|l.unwrap())
        .collect();
    parse(file_name, lines)
}

fn parse(file_name: String, lines: Vec<String>) -> Result<HashMap<String, String>, Error> {
    let mut items = HashMap::new();
    let mut line_no = 1;
    for line in lines {
        let l = line.trim();
        if !l.is_empty() && !l.starts_with("#") {
            let parts = l.split_once("=");
            match parts {
                Some((p1, p2)) => items.insert(p1.to_string(), p2.to_string()),
                None => return Err(Error::new(ErrorKind::InvalidData,
                                              format!("invalid line {} in file {}", file_name, line_no)))
            };
        }
        line_no += 1;
    }
    Ok(items)
}
