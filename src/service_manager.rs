use std::collections::{HashMap, HashSet};
use std::io::{Error, ErrorKind};
use std::thread;
use std::time::Duration;
use yaml_rust::yaml::Hash;
use crate::command_to_run::CommandToRun;
use crate::script::{SCRIPT_STATUS_FINISHED, SCRIPT_STATUS_NOT_STARTED, ScriptChecker};
use crate::service::{build_invalid_script_name_error, Service};
use crate::user_command::WriterWithTCP;
use crate::utilities::{build_invalid_data_error_str, build_invalid_data_error_string};

pub struct Services {
    services: HashMap<String, Service>,
}

pub struct ServiceManager {
    service_sets: HashMap<String, HashSet<String>>,
    services: Services,
    init_command: Option<CommandToRun>,
    shutdown_command: Option<CommandToRun>,
}

impl ScriptChecker for Services {
    fn script_exists(&self, script_name: &String) -> bool {
        if let Ok((service, script_real_name)) = self.get_script_service(script_name) {
            return service.script_exists(script_real_name);
        }
        false
    }

    fn check_scripts(&self, scripts: &HashSet<String>) -> bool {
        scripts.iter()
            .map(|s| self.get_script_status(s))
            .all(|s| s == SCRIPT_STATUS_FINISHED)
    }
}

impl Services {
    fn new(services: &Hash) -> Result<Services, Error> {
        let mut result = Services{ services: HashMap::new() };
        for (name, service) in services {
            let disabled = service["disabled"].as_bool().unwrap_or(false);
            if !disabled {
                let service_name = name.as_str().unwrap().to_string();
                println!("{}", service_name);
                let service = Service::new(service_name.clone(), service, &result)?;
                result.services.insert(service_name, service);
            }
        }
        Ok(result)
    }

    fn get_script_status(&self, script_name: &String) -> usize {
        if let Ok((service, script_real_name)) = self.get_script_service(script_name) {
            return service.get_script_status(&script_real_name);
        }
        SCRIPT_STATUS_NOT_STARTED
    }

    fn find_service(&self, service_name: &String) -> Result<&Service, Error> {
        self.services.get(service_name)
            .map_or_else(||Err(Error::new(ErrorKind::InvalidInput, "service not found")),|s|Ok(s))
    }

    pub fn start_service(&'static self, forced_start: bool, service_name: &String, noexec: bool,
                         writer: &mut WriterWithTCP) -> Result<(), Error> {
        let service = self.find_service(service_name)?;
        service.start(forced_start, self, noexec, writer)
    }

    pub fn start_script(&'static self, forced_start: bool, script_name: &String, noexec: bool,
                        writer: &mut WriterWithTCP) -> Result<(), Error> {
        let (service, script_name) = self.get_script_service(script_name)?;
        service.start_script(&script_name, forced_start, self, noexec, writer)
    }

    pub fn stop_script(&self, script_name: &String, writer: &mut WriterWithTCP) -> Result<(), Error> {
        let (service, script_name) = self.get_script_service(script_name)?;
        service.stop_script(&script_name, writer)
    }

    pub fn stop_service(&self, service_name: &String, noexec: bool, writer: &mut WriterWithTCP) -> Result<(), Error> {
        let service = self.find_service(service_name)?;
        service.stop(noexec, writer)
    }

    fn start_all(&'static self, services: &HashSet<String>, noexec: bool, writer: &mut WriterWithTCP) -> Result<(), Error> {
        for name in services {
            self.services.get(name).unwrap().start(false, self, noexec, writer)?;
        }
        Ok(())
    }

    fn stop_all(&self, noexec: bool, writer: &mut WriterWithTCP) -> Result<(), Error> {
        let mut could_not_stop = Vec::new();
        for (name, service) in &self.services {
            if service.stop(noexec, writer).is_err() {
                could_not_stop.push(name.clone());
            }
        }
        if !could_not_stop.is_empty() {
            return Err(Error::new(ErrorKind::Other,
                                  format!("could not stop these services: {}", could_not_stop.join(","))));
        }
        Ok(())
    }

    pub fn get_script_service(&self, script_name: &String) -> Result<(&Service, String), Error> {
        let parts: Vec<&str> = script_name.split('.').collect();
        if parts.len() != 2 {
            return Err(build_invalid_script_name_error());
        }
        let service = self.find_service(&parts[0].to_string())?;
        Ok((service, parts[1].to_string()))
    }

    fn check_service_name(&self, name: &String) -> Result<(), Error> {
        if self.services.contains_key(name) {
            return Ok(());
        }
        Err(build_invalid_data_error_string(format!("service does not exists: {}", name)))
    }

    pub fn report_status(&self, service_name: Option<&String>) -> String {
        self.services.iter()
            .filter(|(name, _service)|service_name == None || service_name.unwrap() == *name)
            .map(|(name, service)|name.clone() + ":\n" + service.get_status_string().as_str())
            .collect::<Vec<_>>()
            .join("\n")
    }

    pub fn wait_finish(&self) {
        self.services.iter()
            .for_each(|(_name, service)|service.wait_finish())
    }
}

impl ServiceManager {
    pub fn new(service_sets: &Hash, services: &Hash, init_cmd: Option<String>,
               shutdown_cmd: Option<String>, noexec: bool) -> Result<ServiceManager, Error> {
        let services = Services::new(services)?;
        let init_command = match init_cmd {
            Some(cmd) => Some(CommandToRun::new(cmd, None, None, None,None)?),
            None => None
        };
        let shutdown_command = match shutdown_cmd {
            Some(cmd) => Some(CommandToRun::new(cmd, None, None, None,None)?),
            None => None
        };
        let manager = ServiceManager {
            service_sets: build_service_sets(service_sets, &services)?,
            services,
            init_command,
            shutdown_command
        };
        manager.init(noexec)?;
        Ok(manager)
    }

    fn init(&self, noexec: bool) -> Result<(), Error> {
        if let Some(cmd) = &self.init_command {
            println!("Starting init script...");
            cmd.run_sync(noexec)?;
            println!("Finished init script...");
        }
        Ok(())
    }

    pub fn shutdown(&self, noexec: bool, writer: &mut WriterWithTCP) -> Result<(), Error> {
        self.stop_all(noexec, writer)?;
        writer.write_string(format!("Waiting for all services to be finished..."));
        self.services.wait_finish();
        if let Some(cmd) = &self.shutdown_command {
            writer.write_string(format!("Starting shutdown script..."));
            cmd.run_sync(noexec)?;
            writer.write_string(format!("Finished shutdown script..."));
        }
        Ok(())
    }

    pub fn up(&'static self, service_set_name: &String, noexec: bool, writer: &mut WriterWithTCP) -> Result<(), Error> {
        let services = self.service_sets.get(service_set_name)
            .ok_or(Error::new(ErrorKind::InvalidInput, "invalid service set name"))?;
        self.services.start_all(services, noexec, writer)
    }

    pub fn stop_all(&self, noexec: bool, writer: &mut WriterWithTCP) -> Result<(), Error> {
        self.services.stop_all(noexec, writer)
    }

    pub fn start_service(&'static self, forced_start: bool, service_name: &String, noexec: bool,
                         writer: &mut WriterWithTCP) -> Result<(), Error> {
        self.services.start_service(forced_start, service_name, noexec, writer)
    }

    pub fn stop_service(&self, service_name: &String, noexec: bool, writer: &mut WriterWithTCP) -> Result<(), Error> {
        self.services.stop_service(service_name, noexec, writer)
    }

    pub fn start_script(&'static self, forced_start: bool, script_name: &String, noexec: bool,
                        writer: &mut WriterWithTCP) -> Result<(), Error> {
        self.services.start_script(forced_start, script_name, noexec, writer)
    }

    pub fn stop_script(&self, script_name: &String, writer: &mut WriterWithTCP) -> Result<(), Error> {
        self.services.stop_script(script_name, writer)
    }

    pub fn report_status(&self, service_name: Option<&String>) -> String {
        self.services.report_status(service_name)
    }

    pub fn wait_for_scripts(&self, scripts: &HashSet<String>) -> Result<(), Error> {
        for script in scripts {
            if !self.services.script_exists(script) {
                return Err(Error::new(ErrorKind::InvalidInput, format!("Script does not exist: {}", script)));
            }
        }
        let duration = Duration::from_secs(1);
        while !self.services.check_scripts(scripts) {
            thread::sleep(duration);
        }
        Ok(())
    }
}

fn build_service_sets(service_sets: &Hash, service_list: &Services) -> Result<HashMap<String, HashSet<String>>, Error> {
    let mut result: HashMap<String, HashSet<String>> = HashMap::new();
    for (name, service_set) in service_sets {
        let mut services: HashSet<String> = HashSet::new();
        if let Some(includes) = service_set["includes"].as_vec() {
            if includes.is_empty() {
                return Err(build_invalid_data_error_str("empty include directive"));
            }
            for include in includes {
                let another = result.get(include.as_str().unwrap())
                    .ok_or(build_invalid_data_error_str("invalid include service name"))?;
                for item in another {
                    service_list.check_service_name(item)?;
                    services.insert(item.clone());
                }
            }
        }
        let list = service_set["services"].as_vec()
            .ok_or(build_invalid_data_error_str("invalid services directive"))?;
        if list.is_empty() {
            return Err(build_invalid_data_error_str("empty services directive"));
        }
        for item in list {
            let name = item.as_str().unwrap().to_string();
            service_list.check_service_name(&name)?;
            services.insert(name);
        }
        result.insert(name.as_str().unwrap().to_string(), services);
    }
    Ok(result)
}