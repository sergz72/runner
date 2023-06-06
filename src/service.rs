use std::collections::HashMap;
use std::io::{Error, ErrorKind};
use yaml_rust::Yaml;
use crate::command_to_run::CommandToRun;
use crate::script::{Script, SCRIPT_STATUS_NOT_STARTED, ScriptChecker};
use crate::user_command::WriterWithTCP;

pub struct Service {
    name: String,
    post_stop_script: Option<CommandToRun>,
    scripts: HashMap<String, Script>,
}

impl Service {
    pub fn new(service_name: String, service: &Yaml, checker: &dyn ScriptChecker)
               -> Result<Service, Error> {
        if let Some(scripts) = service["scripts"].as_hash() {
            if scripts.is_empty() {
                return Err(build_service_has_no_scripts_error(&service_name));
            }
            let post_stop_script = match service["post-stop-script"].as_str() {
                Some(s) => Some(CommandToRun::new(s.to_string(), None, None, None)?),
                None => None
            };
            let mut result = HashMap::new();
            for (name, script_yaml) in scripts {
                let script_name = name.as_str().unwrap().to_string();
                println!(" - {}", script_name);
                let script = Script::new(script_name.clone(), script_yaml, checker)?;
                result.insert(script_name, script);
            }
            return Ok(Service { name: service_name, post_stop_script, scripts: result });
        }
        Err(build_service_has_no_scripts_error(&service_name))
    }

    pub fn start(&'static self, forced_start: bool, checker: &'static (dyn ScriptChecker + Sync),
                 noexec: bool, writer: &mut WriterWithTCP) -> Result<(), Error> {
        for (_name, script) in &self.scripts {
            script.start(forced_start, checker, noexec, writer)?;
        }
        Ok(())
    }

    pub fn start_script(&'static self, script_name: &String, forced_start: bool, checker: &'static (dyn ScriptChecker + Sync),
                        noexec: bool, writer: &mut WriterWithTCP) -> Result<(), Error> {
        if let Some(script) = self.scripts.get(script_name) {
            return script.start(forced_start, checker, noexec, writer);
        }
        Err(build_invalid_script_name_error())
    }

    pub fn stop_script(&self, script_name: &String, writer: &mut WriterWithTCP) -> Result<(), Error> {
        if let Some(script) = self.scripts.get(script_name) {
            return script.stop(writer);
        }
        Err(build_invalid_script_name_error())
    }

    pub fn stop(&self, noexec: bool, writer: &mut WriterWithTCP) -> Result<(), Error> {
        for (_name, script) in &self.scripts {
            script.stop(writer)?;
        }
        if let Some(script) = &self.post_stop_script {
            writer.write_string(format!("Running post-stop-script for {}", self.name))?;
            script.run_sync(noexec)?;
            writer.write_string(format!("Finished post-stop-script for {}", self.name))?;
        }
        Ok(())
    }

    pub fn script_exists(&self, script_name: String) -> bool {
        self.scripts.contains_key(script_name.as_str())
    }

    pub fn get_script_status(&self, script_name: &String) -> usize {
        self.scripts.get(script_name).map_or_else(||SCRIPT_STATUS_NOT_STARTED, |s|s.get_status())
    }

    pub fn get_status_string(&self) -> String {
        self.scripts.iter()
            .map(|(_name, script)|script.get_status_string())
            .collect::<Vec<_>>()
            .join("\n")
    }
}

pub fn build_invalid_script_name_error() -> Error {
    Error::new(ErrorKind::InvalidInput, "invalid script name")
}

fn build_service_has_no_scripts_error(service_name: &String) -> Error {
    Error::new(ErrorKind::InvalidData, format!("service {} has no scripts", service_name))
}
