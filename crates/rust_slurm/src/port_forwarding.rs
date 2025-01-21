use std::{
    net::{IpAddr, Ipv4Addr, SocketAddr},
    sync::Arc,
};

use anyhow::Error;
use tokio::{
    net::TcpListener,
    task::{self, JoinHandle},
};

pub async fn ssh_port_forwarding<S: AsRef<str>>(
    client: Arc<async_ssh2_tokio::Client>,
    local_addr: S,
    remote_addr: S,
) -> Result<JoinHandle<()>, Error> {
    println!("Got client!");
    let l_addr: SocketAddr = local_addr.as_ref().parse().unwrap();
    let local_listener = TcpListener::bind(l_addr)
        .await
        .expect("Cannot bind local port");
    let arc = std::sync::Arc::new(client);
    let r_addr: SocketAddr = remote_addr.as_ref().parse().unwrap();
    let f = task::spawn(async move {
        loop {
            let (mut socket, _) = local_listener
                .accept()
                .await
                .expect("Cannot process local client");

            println!("Client connected");
            let a = arc.clone();
            tokio::spawn(async move {
                let c = a
                    .open_direct_tcpip_channel(
                        r_addr,
                        SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), 0),
                    )
                    .await;
                match c {
                    Ok(channel) => {
                        let mut ssh_stream = channel.into_stream();

                        match tokio::io::copy_bidirectional(&mut socket, &mut ssh_stream).await {
                            Ok((bytes_to_remote, bytes_to_local)) => {
                                println!(
                            "Connection closed. Sent {} bytes to remote, received {} bytes from remote",
                            bytes_to_remote, bytes_to_local
                        );
                            }
                            Err(e) => eprintln!("Error forwarding traffic: {:?}", e),
                        }
                    }
                    Err(e) => eprintln!("Could not open channel: {:?}", e),
                }
            });
        }
    });
    Ok(f)
}

#[cfg(test)]
mod test {
    use std::sync::Arc;

    use crate::port_forwarding::ssh_port_forwarding;

    #[tokio::test]
    async fn test_port_forwarding() {
        use crate::{login_with_cfg, ConnectionAuth, ConnectionConfig};

        let login_cfg = ConnectionConfig::new(
            ("login23-1.hpc.itc.rwth-aachen.de".to_string(), 22),
            "at325350".to_string(),
            ConnectionAuth::SSHKey {
                path: "/home/aarkue/.ssh/id_ed25519".to_string(),
                passphrase: None,
            },
        );

        let client = login_with_cfg(&login_cfg).await.unwrap();
        let arc = Arc::new(client);
        ssh_port_forwarding(arc, "127.0.0.1:3000", "127.0.0.1:3000")
            .await
            .unwrap();
    }
}
