use std::fs::File;
use std::io::prelude::*;
use std::io::BufReader;
use std::path::Path;

use ron;
use serde::Serialize;

use crate::interpreter::ast;

#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub struct Project {
    pub version: u32,
    pub stmts: Vec<ast::Stmt>,
}

pub fn save<P: AsRef<Path>>(path: P, project: Project) {
    let pretty_config = ron::ser::PrettyConfig::default();
    let mut serializer = ron::ser::Serializer::new(Some(pretty_config), true);
    project
        .serialize(&mut serializer)
        .expect("Failed to serialize project");

    let contents = serializer.into_output_string();
    let mut file = File::create(path).expect("Failed to create project file");

    file.write_all(contents.as_bytes())
        .expect("Failed to write contents of project to file");
}

pub fn open<P: AsRef<Path>>(path: P) -> Project {
    let file = File::open(path).expect("Failed to open project file");
    let buf_reader = BufReader::new(file);

    ron::de::from_reader(buf_reader).expect("Failed to deserialize project file")
}
