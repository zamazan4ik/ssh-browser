use serde::{Deserialize, Serialize};
use ssh2::{FileStat, OpenFlags, OpenType, Session, Sftp};
use std::fs::{DirEntry, File};
use std::io::{Read, Write};
use std::net::TcpStream;
use std::path::Path;

/// Manages SSH and SFTP connections.
pub struct SSHConnection {
    hostname: String,
    username: String,
    password: String,
    port: u16,
    session: Option<Session>,
    sftp: Option<Sftp>,
}
 

impl From<SSHConnectionS> for SSHConnection {
    fn from(conn_s: SSHConnectionS) -> Self {
   
        SSHConnection {
            hostname: conn_s.hostname,
            username: conn_s.username,
            password: conn_s.password,
            port: conn_s.port,
            session: None,
            sftp: None,
         
        }
    }
}
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct SSHConnectionS {
    pub hostname: String,
    pub username: String,
    pub port: u16,
    pub password: String, 
}
impl SSHConnection {
    pub fn from_ssh_connection_file(file_conn: SSHConnectionS) -> Self {
        SSHConnection {
            hostname: file_conn.hostname,
            username: file_conn.username,
            password: file_conn.password,
            port: file_conn.port,
            session: None,
            sftp: None,
        }
    }

    pub fn new(hostname: &str, username: &str, password: &str, port: u16) -> Self {
    
        Self {
            hostname: hostname.to_string(),
            username: username.to_string(),
            password: password.to_string(),
            port,
            session: None,
            sftp: None,
        }
    }

    pub fn connect(&mut self) -> Result<(), String> {
        let addr = format!("{}:{}", self.hostname, self.port);
        let tcp = TcpStream::connect(addr).map_err(|e| format!("Connection error: {}", e))?;
        let mut session = Session::new().map_err(|e| format!("Session creation error: {}", e))?;
        session.set_tcp_stream(tcp);
        session
            .handshake()
            .map_err(|e| format!("Handshake error: {}", e))?;
        session
            .userauth_password(&self.username, &self.password)
            .map_err(|e| format!("Authentication error: {}", e))?;

        if !session.authenticated() {
            return Err("Authentication failed. Check your username and password.".to_string());
        }

        let sftp = session
            .sftp()
            .map_err(|e| format!("SFTP initialization error: {}", e))?;
        self.session = Some(session);
        self.sftp = Some(sftp);

        Ok(())
    }

    pub fn disconnect(&mut self) {
        self.sftp = None;
        self.session = None;
    }

    pub fn delete_file(&self, remote_path: &str) -> Result<(), String> {
        if let Some(sftp) = &self.sftp {
            sftp.unlink(Path::new(remote_path))
                .map_err(|e| format!("Failed to delete file: {}", e))
        } else {
            Err("SFTP subsystem not initialized.".to_string())
        }
    }

    pub fn list_directory(&self, path: &str) -> Result<Vec<(String, bool)>, String> {
        let sftp = self
            .sftp
            .as_ref()
            .ok_or_else(|| "SFTP subsystem not initialized.".to_string())?;

        let entries = sftp
            .readdir(Path::new(path))
            .map_err(|e| format!("Failed to read directory: {}", e))?;

        let mut result = Vec::new();
        for (entry_path, stat) in entries {
            if let Some(name) = entry_path.file_name() {
                let name_str = name.to_string_lossy().to_string();
                result.push((name_str, stat.is_dir()));
            }
        }

        result.sort_by(|a, b| {
            if a.1 && !b.1 {
                std::cmp::Ordering::Less
            } else if !a.1 && b.1 {
                std::cmp::Ordering::Greater
            } else {
                a.0.cmp(&b.0)
            }
        });

        Ok(result)
    }

    pub fn read_file(&self, remote_path: &str) -> Result<String, String> {
        if let Some(sftp) = &self.sftp {
            let mut file = sftp
                .open(Path::new(remote_path))
                .map_err(|e| format!("Failed to open file: {}", e))?;
            let mut content = String::new();
            file.read_to_string(&mut content)
                .map_err(|e| format!("Failed to read file: {}", e))?;
            Ok(content)
        } else {
            Err("SFTP subsystem not initialized.".to_string())
        }
    }

    pub fn write_file(&self, remote_path: &str, content: &str) -> Result<(), String> {
        if let Some(sftp) = &self.sftp {
            let mut file = sftp
                .create(Path::new(remote_path))
                .map_err(|e| format!("Failed to create file: {}", e))?;
            file.write_all(content.as_bytes())
                .map_err(|e| format!("Failed to write file: {}", e))?;
            Ok(())
        } else {
            Err("SFTP subsystem not initialized.".to_string())
        }
    }

   

    pub fn download_file(&self, remote_path: &str, local_path: &str) -> Result<(), String> {
        let sftp = self
            .sftp
            .as_ref()
            .ok_or_else(|| "SFTP subsystem not initialized.".to_string())?;
        let mut remote_file = sftp
            .open(Path::new(remote_path))
            .map_err(|e| format!("Failed to open remote file: {}", e))?;
        let mut local_file = std::fs::File::create(local_path)
            .map_err(|e| format!("Failed to create local file: {}", e))?;

        let mut buffer = [0; 8192];
        loop {
            let bytes_read = remote_file
                .read(&mut buffer)
                .map_err(|e| format!("Error reading from remote file: {}", e))?;
            if bytes_read == 0 {
                break;
            }
            local_file
                .write_all(&buffer[..bytes_read])
                .map_err(|e| format!("Error writing to local file: {}", e))?;
        }
        Ok(())
    }

    pub fn upload_file(&self, local_path: &str, remote_path: &str) -> Result<(), String> {
        let sftp = self
            .sftp
            .as_ref()
            .ok_or_else(|| "SFTP subsystem not initialized.".to_string())?;
        let mut local_file = std::fs::File::open(local_path)
            .map_err(|e| format!("Failed to open local file: {}", e))?;
        let mut remote_file = sftp
            .open_mode(
                Path::new(remote_path),
                OpenFlags::WRITE | OpenFlags::CREATE | OpenFlags::TRUNCATE,
                0o644,
                OpenType::File,
            )
            .map_err(|e| format!("Failed to open remote file: {}", e))?;

        let mut buffer = [0; 8192];
        loop {
            let bytes_read = local_file
                .read(&mut buffer)
                .map_err(|e| format!("Error reading from local file: {}", e))?;
            if bytes_read == 0 {
                break;
            }
            remote_file
                .write_all(&buffer[..bytes_read])
                .map_err(|e| format!("Error writing to remote file: {}", e))?;
        }
        Ok(())
    }
}

fn is_directory(stat: &FileStat) -> bool {
    if let Some(perms) = stat.perm {
        perms & libc::S_IFDIR as u32 != 0
    } else {
        false
    }
}
