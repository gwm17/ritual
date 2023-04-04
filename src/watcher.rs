use notify::{Watcher, RecommendedWatcher, Config};
use notify::event::Event;
use tokio::sync::mpsc::Sender;

/*
    create_watcher wraps Notify initialization. Note the use of blocking_send to bridge the synchronous code.
    Requires a sender for Notify::Events. 
 */
pub fn create_watcher(queue: Sender<Event>) -> Result<Box<dyn Watcher>, notify::Error> {

    //Notify uses a closure callback to handle events
    let ritual = Box::new(RecommendedWatcher::new(move |event| 
        {
            if let Ok(data) = event {
                tracing::trace!("Received an event: {:?}", data);
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