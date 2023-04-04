mod server;
mod watcher;
mod project;
mod file;
mod message;
mod config;

use bytes::Bytes;
use server::run_server;
use watcher::create_watcher;
use project::Project;
use config::{Config, read_config_file};

fn print_help() {
    print!("Ritual is run as:\ncargo -r run -- <your_config>\nThe config file is a yaml file which contains the server address and project directory\n");
}

fn get_config(arg: &str) -> Option<Config> {
    if arg == "--help" {
        print_help();
        return None;
    }

    let config = match read_config_file(&std::path::Path::new(arg)) {
        Ok(c) => c,
        Err(e) => {
            println!("Err {}", e);
            return None;
        }
    };
    Some(config)
}

#[tokio::main]
async fn main() {

    let subscriber = tracing_subscriber::fmt()
        .compact()
        .with_file(true)
        .with_line_number(true)
        .with_thread_ids(true)
        .with_target(false)
        .with_max_level(tracing::Level::TRACE)
        .finish();

    tracing::subscriber::set_global_default(subscriber).expect("Error occured setting up tracing!");

    let args: Vec<String> = std::env::args().collect();
    if args.len() != 2 {
        tracing::error!("Ritual requires an input yaml file!");
        print_help();
    }

    let config = match get_config(&args[1]) {
        Some(c) => c,
        None => return
    };

    let (data_sender, data_reciever) = tokio::sync::mpsc::channel::<Bytes>(10);
    let (event_sender, event_reciever) = tokio::sync::mpsc::channel::<notify::event::Event>(5);
    let (shutdown_sender, mut shutdown_reciever) = tokio::sync::mpsc::channel::<i32>(1);

    match run_server(&config.server_address, data_reciever).await {
        Ok(_) => {},
        Err(e) => {
            tracing::error!("Server initialization error: {}", e);
            return;
        }
    };

    let mut project = match Project::new(&config.project_directory, event_reciever, data_sender) {
        Ok(p) => p,
        Err(e) => {
            tracing::error!("Project initialization error: {}", e);
            return
        }
    };

    tokio::spawn(async move {
        match project.handle_events().await {
            Ok(_) => {},
            Err(e) => tracing::error!("Project error: {}", e)
        }
    });

    tokio::spawn(async move {
        match tokio::signal::ctrl_c().await {
            Ok(()) => {
                tracing::info!("Recieved a ctrl-c, shutting down.");
                shutdown_sender.send(1).await.unwrap();
            }
            Err(e) => {
                tracing::error!("Ctrl-c error: {}", e);
            }
        }
    });

    let result = tokio::task::spawn_blocking(move || {
            let mut watcher = match create_watcher(event_sender) {
                Ok(w) => w,
                Err(e) => {
                    tracing::error!("Notify error: {}", e);
                    return
                }
            };

            match watcher.watch(&std::path::Path::new("test_project"), notify::RecursiveMode::Recursive) {
                Err(e) => {
                    tracing::error!("Notify error: {}", e);
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
