use std::collections::HashSet;
use std::fmt::{Display, Formatter};
use std::fs::File;
use std::{env, io};
use std::io::{Error, Write};
use std::process::{Child, Command, Stdio};
use env_file::parse_env_file;
use split_string::split_string;
use crate::utilities::build_invalid_data_error_str;

pub struct CommandToRun {
    command: String,
    parameters: Vec<String>,
    log_file_out: Option<String>,
    log_file_err: Option<String>,
    work_dir: Option<String>,
    env_file: Option<String>,
}

fn format_vector(vector: &Vec<String>) -> String {
    if vector.is_empty() {
        return "[]".to_string();
    }
    "[\"".to_string() + vector.join("\",\"").as_str() + "\"]"
}

impl Display for CommandToRun {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "CommandToRun {} parameters={} log_file_out={} log_file_err={} work_dir={} env_file={}",
               self.command,
               format_vector(&self.parameters),
               if self.log_file_out.is_none() { "None" } else { self.log_file_out.as_ref().unwrap().as_str() },
               if self.log_file_err.is_none() { "None" } else { self.log_file_err.as_ref().unwrap().as_str() },
               if self.work_dir.is_none() { "None" } else { self.work_dir.as_ref().unwrap().as_str() },
               if self.env_file.is_none() { "None" } else { self.env_file.as_ref().unwrap().as_str() })
    }
}

impl CommandToRun {
    pub fn new(command: String, logfile_out: Option<String>, logfile_err: Option<String>, workdir: Option<String>,
               env_file: Option<String>) -> Result<CommandToRun, Error> {
        if command.is_empty() {
            return Err(build_invalid_data_error_str("command is empty"));
        }
        let work_dir = match workdir {
            Some(wd) => Some(CommandToRun::build_file_path(&wd, &None)?),
            None => None
        };
        let parts: Vec<String> = split_string(command, HashSet::from(['"']))?
            .iter()
            .map(|p| p.result.clone())
            .collect();
        let name = CommandToRun::build_file_path(&parts[0], &work_dir)?;
        let log_file_out = match logfile_out {
            Some(f) => Some(CommandToRun::build_file_path(&f, &work_dir)?),
            None => None
        };
        let log_file_err = match logfile_err {
            Some(f) => Some(CommandToRun::build_file_path(&f, &work_dir)?),
            None => None
        };
        let mut parameters: Vec<String> = Vec::new();
        for part in parts.iter().skip(1) {
            parameters.push(CommandToRun::build_file_path(part, &work_dir)?);
        }
        Ok(CommandToRun {
            command: name,
            parameters,
            log_file_out,
            log_file_err,
            work_dir,
            env_file,
        })
    }

    fn prepare(&self) -> Result<Command, Error> {
        let mut command = Command::new(&self.command);
        command.args(&self.parameters);
        if let Some(f) = &self.env_file {
            let env_variables = parse_env_file(CommandToRun::build_file_path(&f, &self.work_dir)?)?;
            command.envs(&env_variables);
        }
        if let Some(work_dir) = &self.work_dir {
            command.current_dir(work_dir);
        }
        if let Some(log_file) = &self.log_file_out {
            command.stdout(Stdio::from(File::create(log_file)?));
        }
        if let Some(log_file) = &self.log_file_err {
            command.stderr(Stdio::from(File::create(log_file)?));
        }
        Ok(command)
    }

    pub fn run_sync(&self, noexec: bool) -> Result<(), Error> {
        if noexec {
            println!("{}", self);
            return Ok(());
        }
        let mut command = self.prepare()?;
        let output = command.output()?;
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

    pub fn build_file_path(path: &String, work_dir: &Option<String>) -> Result<String, Error> {
        let cwd = env::current_dir()?;
        let mut result = path.replace("$PWD", &cwd.display().to_string());
        result = result.replace("~", &env::var("HOME").unwrap());
        if let Some(wd) = work_dir {
            result = result.replace("$WD", wd);
        }
        Ok(result)
    }
}