mod server;
mod watcher;
mod project;

use bytes::Bytes;
use std::sync::Arc;
use tokio::sync::Mutex;

use server::{ServerListener, ConnectionHandler, ServerSender, ConnectionList};
use watcher::create_watcher;
use project::Project;

#[tokio::main]
async fn main() {

    let (data_sender, data_reciever) = tokio::sync::mpsc::channel::<Bytes>(10);
    let (event_sender, event_reciever) = tokio::sync::mpsc::channel::<notify::event::Event>(5);
    let (shutdown_sender, mut shutdown_reciever) = tokio::sync::mpsc::channel::<i32>(1);
    let connections = Arc::new(Mutex::new(ConnectionList::new()));

    let (mut listener, conn_reciever) = match ServerListener::startup("127.0.0.1:52324").await {
        Ok(start) => start,
        Err(e) => {
            println!("{}", e); 
            return;
        }
    };

    let mut conn_handler = ConnectionHandler::new(conn_reciever, connections.clone());
    let mut sender = ServerSender::new(data_reciever, connections.clone());
    let mut project = match Project::new(std::path::Path::new("test_project"), event_reciever, data_sender) {
        Ok(p) => p,
        Err(e) => {
            println!("Project initialization error: {}", e);
            return
        }
    };

    tokio::spawn(async move { 
        match listener.wait_for_connection().await {
            Ok(_) => {},
            Err(e) => {
                println!("{}", e);
            }
        } 
    });

    tokio::spawn(async move {
        match conn_handler.recieve_connection().await {
            Ok(_) => {},
            Err(e) => println!("{}", e)
        }
    });

    tokio::spawn(async move {
        match sender.wait_for_data().await {
            Ok(_) => {},
            Err(e) => println!("{}", e)
        }
    });

    tokio::spawn(async move {
        match project.handle_events().await {
            Err(e) => println!("{}", e),
            _ => {}
        }
    });

    tokio::spawn(async move {
        match tokio::signal::ctrl_c().await {
            Ok(()) => {
                println!("Recieved a ctrl-c, shutting down.");
                shutdown_sender.send(1).await.unwrap();
            }
            Err(e) => {
                println!("Ctrl-c error! {}", e);
            }
        }
    });

    let result = tokio::task::spawn_blocking(move || {
            let mut watcher = match create_watcher(event_sender) {
                Ok(w) => w,
                Err(e) => {
                    println!("Notify error: {}", e);
                    return
                }
            };

            match watcher.watch(&std::path::Path::new("test_project"), notify::RecursiveMode::Recursive) {
                Err(e) => {
                    println!("Notify error: {}", e);
                    return
                }
                _ => {}
            }

            loop {
                _ = shutdown_reciever.blocking_recv().unwrap();
                break;
            }
    });

    result.await.unwrap();

}
