use notify::{Watcher, RecommendedWatcher, Config};
use notify::event::Event;
use tokio::sync::mpsc::Sender;


pub fn create_watcher(queue: Sender<Event>) -> Result<Box<dyn Watcher>, notify::Error> {
    let ritual = Box::new(RecommendedWatcher::new(move |event| 
        {
            if let Ok(data) = event {
                tracing::info!("Received an event: {:?}", data);
                match queue.blocking_send(data) {
                    Ok(()) => { tracing::trace!("Sent!")},
                    Err(_) => tracing::error!("RitualWatcher ran into an error trying to send an event!")
                };
            }
        }, 
        Config::default()
    )?);
    Ok(ritual)
}