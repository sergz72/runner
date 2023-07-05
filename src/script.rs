use std::collections::HashSet;
use std::io::{Error, ErrorKind};
use std::net::TcpStream;
use std::str::FromStr;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::mpsc::{channel, Receiver, Sender, TryRecvError};
use std::sync::Mutex;
use std::thread;
use std::thread::sleep;
use std::time::Duration;
use yaml_rust::Yaml;
use crate::command_to_run::CommandToRun;
use crate::user_command::WriterWithTCP;
use crate::utilities::build_invalid_data_error_string;

pub const SCRIPT_STATUS_NOT_STARTED: usize = 0;
pub const SCRIPT_STATUS_STARTING: usize = 1;
pub const SCRIPT_STATUS_RUNNING: usize = 2;
pub const SCRIPT_STATUS_INTERRUPTED: usize = 3;
pub const SCRIPT_STATUS_FINISHED: usize = 4;
pub const SCRIPT_STATUS_FAILED: usize = 5;
pub const SCRIPT_STATUS_KILLED: usize = 6;

pub trait ScriptChecker {
    fn script_exists(&self, script_name: &String) -> bool;
    fn check_scripts(&self, scripts: &HashSet<String>) -> bool;
}

pub struct Script {
    name: String,
    command: CommandToRun,
    wait_for_ports: HashSet<(String, u16)>,
    wait_until_scripts_are_done: HashSet<String>,
    delay: Option<Duration>,
    status: AtomicUsize,
    tx: Mutex<Sender<()>>,
    rx: Mutex<Receiver<()>>,
}

fn process_host_port(host_port: &str, name: &String) -> Result<(String, u16), Error> {
    let splitted: Vec<&str> = host_port.split(':').collect();
    let (host, port) = match splitted.len()  {
        1 => ("localhost", splitted[0]),
        2 => (splitted[0], splitted[1]),
        _ => return Err(build_invalid_data_error_string(
            format!("more than one : in wait_for_ports in script {}", name)))
    };
    if let Ok(p) = isize::from_str(port) {
        if p <= 0 || p > 65535 {
            Err(build_invalid_data_error_string(
                format!("port value is out of range is invalid in script {}", name)))
        } else {
            Ok((host.to_string(), p as u16))
        }
    } else {
        Err(build_invalid_data_error_string(
            format!("port number is invalid in wait_for_ports in script {}", name)))
    }
}

impl Script {
    pub fn new(name: String, items: &Yaml, checker: &dyn ScriptChecker) -> Result<Script, Error> {
        let work_dir = items["workdir"].as_str().map(|s| s.to_string());
        let env_file = items["env_file"].as_str().map(|s| s.to_string());
        let log_file_out = items["log_file"].as_str().map(|s| s.to_string());
        let log_file_err = items["log_file_err"].as_str().map(|s| s.to_string());
        let command = match items["command"].as_str() {
            Some(c) => CommandToRun::new(c.to_string(), log_file_out,
                                         log_file_err, work_dir, env_file)?,
            None => return Err(build_invalid_data_error_string(format!("script {} has no command", name)))
        };
        let mut wait_for_ports = HashSet::new();
        if let Some(wait_ports) = items["wait_for_ports"].as_vec() {
            for port_yaml in wait_ports {
                if let Some(host_port) = port_yaml.as_str() {
                    wait_for_ports.insert(process_host_port(host_port, &name)?);
                } else {
                    return Err(build_invalid_data_error_string(
                        format!("wait_for_ports is invalid in script {}", name)));
                }
            }
        }
        let wait_until_scripts_are_done = items["wait_until_scripts_are_done"].as_vec()
            .map(|v| v.iter().map(|i| i.as_str().unwrap().to_string()).collect::<HashSet<_>>())
            .unwrap_or(HashSet::new());
        if !wait_until_scripts_are_done.iter()
            .all(|s| checker.script_exists(s)) {
            return Err(build_invalid_data_error_string(
                                  format!("wait_until_scripts_are_done is invalid in script {}", name)));
        }
        let delay = items["delay"].as_i64().map(|d|Duration::from_secs(d as u64));
        let (tx, rx): (Sender<()>, Receiver<()>) = channel();
        Ok(Script {
            name,
            command,
            wait_for_ports,
            wait_until_scripts_are_done,
            delay,
            status: AtomicUsize::new(SCRIPT_STATUS_NOT_STARTED),
            tx: Mutex::new(tx),
            rx: Mutex::new(rx),
        })
    }

    pub fn start(&'static self, forced_start: bool, checker: &'static (dyn ScriptChecker + Sync),
                 noexec: bool, writer: &mut WriterWithTCP) -> Result<(), Error> {
        let status = self.status.load(Ordering::Relaxed);
        if status == SCRIPT_STATUS_NOT_STARTED || status == SCRIPT_STATUS_INTERRUPTED ||
            status == SCRIPT_STATUS_FINISHED || status == SCRIPT_STATUS_KILLED || status == SCRIPT_STATUS_FAILED {
            self.status.store(SCRIPT_STATUS_STARTING, Ordering::Relaxed);
            writer.write_string(format!("Starting {}...", self.name));
            if forced_start {
                if noexec {
                    self.run_noexec();
                } else {
                    thread::spawn(|| {
                        self.run_exec();
                    });
                }
            } else {
                if noexec {
                    thread::spawn(|| {
                        if !self.wait_for_ports(&self.wait_for_ports) {
                            return;
                        }
                        if !self.wait_for_scripts(&self.wait_until_scripts_are_done, checker) {
                            return;
                        }
                        if let Some(d) = self.delay {
                            sleep(d);
                        }
                        self.run_noexec();
                    });
                } else {
                    thread::spawn(|| {
                        if !self.wait_for_ports(&self.wait_for_ports) {
                            return;
                        }
                        if !self.wait_for_scripts(&self.wait_until_scripts_are_done, checker) {
                            return;
                        }
                        if let Some(d) = self.delay {
                            sleep(d);
                        }
                        self.run_exec();
                    });
                }
            }
        }
        Ok(())
    }

    fn run_exec(&self) {
        self.run(false)
    }

    fn run_noexec(&self) {
        self.run(true)
    }

    fn run(&self, noexec: bool) {
        match self.command.run_async(noexec) {
            Ok(Some(mut child)) => {
                self.status.store(SCRIPT_STATUS_RUNNING, Ordering::Relaxed);
                println!("Started {}...", self.name);
                let duration = Duration::from_millis(100);
                loop {
                    match child.try_wait() {
                        Ok(Some(status)) => {
                            self.status.store(SCRIPT_STATUS_FINISHED, Ordering::Relaxed);
                            println!("Finished {} with exitcode {}", self.name, status);
                            break;
                        }
                        Ok(None) => {
                            if !self.wait(duration) {
                                child.kill().unwrap();
                                break;
                            }
                        }
                        Err(e) => {
                            child.kill().unwrap();
                            self.status.store(SCRIPT_STATUS_FAILED, Ordering::Relaxed);
                            println!("Failed to wait {}: {}", self.name, e);
                            break;
                        }
                    }
                }
            }
            Ok(None) => {
                self.status.store(SCRIPT_STATUS_FINISHED, Ordering::Relaxed);
                println!("Finished {} with noexec", self.name);
            }
            Err(e) => {
                self.status.store(SCRIPT_STATUS_FAILED, Ordering::Relaxed);
                println!("Failed to start {}: {}", self.name, e);
            }
        }
    }

    pub fn stop(&self, writer: &mut WriterWithTCP) -> Result<(), Error> {
        writer.write_string(format!("Stopping {}...", self.name));
        match self.status.load(Ordering::Relaxed) {
            SCRIPT_STATUS_STARTING | SCRIPT_STATUS_RUNNING => self.interrupt(),
            _ => Ok(())
        }
    }

    fn interrupt(&self) -> Result<(), Error> {
        self.tx.lock().unwrap().send(()).map_err(|_e| Error::new(ErrorKind::Other, "send error"))
    }

    pub fn get_status(&self) -> usize {
        self.status.load(Ordering::Relaxed)
    }

    fn wait_for_scripts(&self, scripts: &HashSet<String>, checker: &dyn ScriptChecker) -> bool {
        let duration = Duration::from_secs(1);
        while !checker.check_scripts(scripts) {
            if !self.wait(duration) {
                return false;
            }
        }
        true
    }

    fn wait(&self, duration: Duration) -> bool {
        match self.rx.lock().unwrap().try_recv() {
            Ok(_data) => {
                self.status.store(SCRIPT_STATUS_INTERRUPTED, Ordering::Relaxed);
                return false;
            }
            Err(TryRecvError::Empty) => sleep(duration),
            Err(TryRecvError::Disconnected) => {
                self.status.store(SCRIPT_STATUS_INTERRUPTED, Ordering::Relaxed);
                return false;
            }
        }
        true
    }

    fn wait_for_ports(&self, ports: &HashSet<(String, u16)>) -> bool {
        let duration = Duration::from_secs(1);
        while !ports.iter().all(|(host, port)| TcpStream::connect((host.as_str(), *port)).is_ok()) {
            if !self.wait(duration) {
                return false;
            }
        }
        true
    }

    pub fn get_status_string(&self) -> String {
        let status_string = match self.status.load(Ordering::Relaxed) {
            SCRIPT_STATUS_STARTING => "starting",
            SCRIPT_STATUS_KILLED => "killed",
            SCRIPT_STATUS_FAILED => "failed",
            SCRIPT_STATUS_FINISHED => "finished",
            SCRIPT_STATUS_NOT_STARTED => "not started",
            SCRIPT_STATUS_INTERRUPTED => "interrupted",
            SCRIPT_STATUS_RUNNING => "running",
            _ => "unknown"
        };
        format!("  {}: {}", self.name, status_string)
    }

    pub fn wait_finish(&self) {
        let delay = Duration::from_millis(100);
        loop {
            let status = self.get_status();
            if status != SCRIPT_STATUS_RUNNING && status != SCRIPT_STATUS_STARTING {
                break;
            }
            sleep(delay);
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::script::process_host_port;

    fn check_host_port(host: &str, port: u16, input: &str, name: &String) {
        let result = process_host_port(&input.to_string(), name);
        assert!(result.is_ok());
        let (h, p) = result.unwrap();
        assert_eq!(h.as_str(), host);
        assert_eq!(p, port);
    }

    #[test]
    fn test_process_host_port() {
        let name = "test".to_string();
        check_host_port("localhost", 1234, "1234", &name);
        check_host_port("server", 1234, "server:1234", &name);
        assert!(process_host_port(&"aaaa", &name).is_err());
        assert!(process_host_port(&"123456", &name).is_err());
        assert!(process_host_port(&"aaaa:123456", &name).is_err());
        assert!(process_host_port(&"aaaa:1234:", &name).is_err());
    }
}