#[cfg(test)]
use crate::ConnectionConfig;


#[cfg(feature = "ssh")]
/// SSH Port Forwarding
pub mod port_forwarding;


#[cfg(test)]
pub (crate) fn get_config_from_env() -> ConnectionConfig{
    use std::env;

    use crate::ConnectionAuth;


    let host = env::var_os("HOSTNAME").unwrap().to_string_lossy().to_string();
    let port =  env::var_os("PORT").unwrap().to_string_lossy().parse().unwrap();
    let username =  env::var_os("USERNAME").unwrap().to_string_lossy().to_string();
    let ssh_key_path =  env::var_os("SSH_KEY_PATH").unwrap().to_string_lossy().to_string();
    let ssh_key_password =  env::var_os("SSH_KEY_PASSWORD").map(|p| p.to_string_lossy().to_string());
    
    ConnectionConfig::new(
        (host, port),
        username,
        ConnectionAuth::SSHKey {
            path: ssh_key_path,
            passphrase: ssh_key_password,
        },
    )
}