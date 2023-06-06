use std::collections::HashMap;
use std::fmt::{Display, Formatter};
use std::fs::File;
use std::{env, io};
use std::io::{Error, ErrorKind, Write};
use std::os::fd::{AsRawFd, FromRawFd};
use std::process::{Child, Command, Stdio};
use std::sync::RwLock;
use crate::env_file::parse_env_file;

pub struct CommandToRun {
    command: String,
    parameters: Vec<String>,
    log_file: Option<String>,
    work_dir: Option<String>,
    env_variables: HashMap<String, String>,
    file: RwLock<Option<File>>,
}

fn format_vector(vector: &Vec<String>) -> String {
    if vector.is_empty() {
        return "[]".to_string();
    }
    "[\"".to_string() + vector.join("\",\"").as_str() + "\"]"
}

fn format_map(map: &HashMap<String, String>) -> String {
    if map.is_empty() {
        return "[]".to_string();
    }
    let vector: Vec<String> = map.iter().map(|(k, v)| k.clone() + ":" + v).collect();
    "\n[".to_string() + vector.join("\n").as_str() + "]"
}

impl Display for CommandToRun {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "CommandToRun {} parameters={} log_file={} work_dir={} env_variables={}",
               self.command,
               format_vector(&self.parameters),
               if self.log_file.is_none() { "None" } else { self.log_file.as_ref().unwrap().as_str() },
               if self.work_dir.is_none() { "None" } else { self.work_dir.as_ref().unwrap().as_str() },
               format_map(&self.env_variables))
    }
}

impl CommandToRun {
    pub fn new(command: String, logfile: Option<String>, workdir: Option<String>,
               env_file: Option<String>) -> Result<CommandToRun, Error> {
        if command.is_empty() {
            return Err(Error::new(ErrorKind::InvalidData, "command is empty"));
        }
        let work_dir = match workdir {
            Some(wd) => Some(CommandToRun::build_file_path(wd, &None)?),
            None => None
        };
        let env_variables = match env_file {
            Some(f) => parse_env_file(CommandToRun::build_file_path(f, &work_dir)?)?,
            None => HashMap::new()
        };
        let mut parts = command.split(' ');
        let name = CommandToRun::build_file_path(parts.next().unwrap().to_string(),
                                                 &work_dir)?;
        let log_file = match logfile {
            Some(f) => Some(CommandToRun::build_file_path(f, &work_dir)?),
            None => None
        };
        let mut parameters = Vec::new();
        while let Some(part) = parts.next() {
            parameters.push(CommandToRun::build_file_path(part.to_string(), &work_dir)?);
        }
        Ok(CommandToRun {
            command: name,
            parameters,
            log_file,
            work_dir,
            env_variables,
            file: RwLock::new(None),
        })
    }

    fn prepare(&self) -> Result<Command, Error> {
        let mut command = Command::new(&self.command);
        command.args(&self.parameters)
            .envs(&self.env_variables);
        if let Some(work_dir) = &self.work_dir {
            command.current_dir(work_dir);
        }
        if let Some(log_file) = &self.log_file {
            let file = File::create(log_file)?;
            command.stdout(unsafe { Stdio::from_raw_fd(file.as_raw_fd()) });
            command.stderr(unsafe { Stdio::from_raw_fd(file.as_raw_fd()) });
            *self.file.write().unwrap() = Some(file);
        }
        Ok(command)
    }

    pub fn file_close(&self) {
        *self.file.write().unwrap() = None;
    }

    pub fn run_sync(&self, noexec: bool) -> Result<(), Error> {
        if noexec {
            println!("{}", self);
            return Ok(());
        }
        let mut command = self.prepare()?;
        let output = command.output()?;
        self.file_close();
        io::stdout().write_all(&output.stdout)?;
        io::stderr().write_all(&output.stderr)
    }

    pub fn run_async(&self, noexec: bool) -> Result<Option<Child>, Error> {
        if noexec {
            println!("{}", self);
            return Ok(None);
        }
        let mut command = self.prepare()?;
        command.spawn().map(|r| Some(r))
    }

    pub fn build_file_path(path: String, work_dir: &Option<String>) -> Result<String, Error> {
        let cwd = env::current_dir()?;
        let mut result = path.replace("$PWD", &cwd.display().to_string());
        result = result.replace("~", &env::var("HOME").unwrap());
        if let Some(wd) = work_dir {
            result = result.replace("$WD", wd);
        }
        Ok(result)
    }
}