use std::fmt::Display;
use std::error::Error;
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::net::{TcpListener, TcpStream};
use tokio::io::AsyncWriteExt;
use tokio::sync::{Mutex, mpsc::{Receiver, Sender, channel}};
use bytes::Bytes;


#[derive(Debug)]
pub enum ServerError {
    StartupError(std::io::Error),
    SendError(tokio::sync::mpsc::error::SendError<Connection>),
    ConnectionError(std::io::Error, String)
}

impl Display for ServerError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::StartupError(x) => write!(f, "Server ran into an error on startup: {}", x),
            Self::SendError(e) => write!(f, "Server ran into a send error: {}\n", e),
            Self::ConnectionError(e, address) => write!(f, "Server ran into a connection error: {}\n Address of connection: {}", e, address)
        }
    }
}

impl From<tokio::sync::mpsc::error::SendError<Connection>> for ServerError {
    fn from(value: tokio::sync::mpsc::error::SendError<Connection>) -> Self {
        Self::SendError(value)
    }
}

impl Error for ServerError {

}

#[derive(Debug)]
pub struct Connection {
    stream: TcpStream,
    address: SocketAddr,
    is_open: bool
}

impl Connection {

    pub fn new(stream: TcpStream, addr: SocketAddr) -> Self {
        Connection{ stream: stream, address: addr, is_open: true}
    }

    pub fn is_open(&self) -> &bool {
        &self.is_open
    }

    pub async fn write(&mut self, data: &Bytes) -> Result<(), ServerError> {
        match self.stream.write_all(data).await {
            Ok(()) => Ok(()),
            Err(e) => { 
                self.is_open = false;
                Err(ServerError::ConnectionError(e, self.address.to_string()))
            }
        }
    }
}

pub type ConnectionList = Vec<Connection>;

#[derive(Debug)]
pub struct ServerListener {
    listener: TcpListener,
    connection_queue: Sender<Connection>,
    address: SocketAddr
}

impl ServerListener {
    
    //Startup server by spawning a listener port
    pub async fn startup(addr: &str) -> Result<(ServerListener, Receiver<Connection>), ServerError> {
        let (tx, rx) = channel(5);
        let listener = match TcpListener::bind(addr).await {
            Ok(net) => ServerListener { listener: net, connection_queue: tx, address: addr.parse().unwrap() },
            Err(e) => return Err(ServerError::StartupError(e))
        };
        tracing::info!("Server listening at address: {}", listener.address);
        Ok((listener, rx))
    }

    pub async fn wait_for_connection(&mut self) -> Result<(), ServerError> {
        
        loop {
            let (stream, address) = match self.listener.accept().await {
                Ok(cxn) => {
                    tracing::info!("Connected to client at {}", cxn.1);
                    cxn
                },
                Err(e) => return Err(ServerError::StartupError(e))
            };

            self.connection_queue.send(Connection::new(stream, address)).await?
        }
    }
}

#[derive(Debug)]
pub struct ConnectionHandler {
    connection_queue: Receiver<Connection>,
    connections: Arc<Mutex<ConnectionList>>
}

impl ConnectionHandler {

    pub fn new(conn_queue: Receiver<Connection>, conns: Arc<Mutex<ConnectionList>>) -> ConnectionHandler {
        ConnectionHandler {
            connection_queue: conn_queue,
            connections: conns
        }
    }

    pub async fn recieve_connection(&mut self) -> Result<(), ServerError> {
        loop {
            match self.connection_queue.recv().await {
                Some(cxn) => {
                    let mut list = self.connections.lock().await;
                    if list.len() == 5 {
                        tracing::warn!("Max number of connections (5) reached, cannot connect");
                    } else  {
                       list.push(cxn);
                    }
                }
                None => {
                    tracing::info!("Listener was closed");
                    break;
                }
            }
        }
        Ok(())
    }
}

#[derive(Debug)]
pub struct ServerSender {
    data_queue: Receiver<Bytes>,
    connections: Arc<Mutex<ConnectionList>>
}
impl ServerSender {

    pub fn new(queue: Receiver<Bytes>, conns: Arc<Mutex<ConnectionList>>) -> Self {
        ServerSender { data_queue: queue, connections: conns }
    }

    pub async fn wait_for_data(&mut self) -> Result<(), ServerError> {
        loop {
            match self.data_queue.recv().await {
                Some(data) => {
                    let mut list = self.connections.lock().await;
                    for cxn in list.iter_mut() {
                        match cxn.write(&data).await {
                            Ok(()) => {},
                            Err(e) => {
                                tracing::info!("Connection {} recieved the following error: {}. Closing connection.", cxn.address, e);
                            }
                        };
                    }

                    list.retain(|cxn| { *cxn.is_open() })
                },
                None =>  {
                    tracing::info!("Sender closed at ServerSender::wait_for_data");
                    break
                }
            }
        }
        Ok(())
    }

}

pub async fn run_server(address: &str, data_reciever: Receiver<Bytes>) -> Result<(), ServerError> {
    let connections = Arc::new(Mutex::new(ConnectionList::new()));
    let (mut listener, conn_reciever) = ServerListener::startup(address).await?;
    let mut conn_handler = ConnectionHandler::new(conn_reciever, connections.clone());
    let mut sender = ServerSender::new(data_reciever, connections.clone());

    tokio::spawn(async move { 
        match listener.wait_for_connection().await {
            Ok(_) => {},
            Err(e) => tracing::error!("Listener error: {}", e)
        } 
    });

    tokio::spawn(async move {
        match conn_handler.recieve_connection().await {
            Ok(_) => {},
            Err(e) => tracing::error!("ConnectionHandler error: {}", e)
        }
    });

    tokio::spawn(async move {
        match sender.wait_for_data().await {
            Ok(_) => {},
            Err(e) => tracing::error!("Sender error: {}", e)
        }
    });

    Ok(())
}