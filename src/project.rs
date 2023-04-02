use tokio::sync::mpsc::{Receiver, Sender};
use bytes::Bytes;
use std::path::{Path, PathBuf};
use notify::event::{Event, EventKind, CreateKind, ModifyKind};

use crate::file::{CompassFile, CompassFileError};
use crate::message::{Message, convert_messages_to_bytes};

const COMPASS_BINARY_EXT: &str = ".BIN";

#[derive(Debug)]
pub enum ProjectError {
    ProjectDirError,
    RunDirError,
    ProjectIOError(std::io::Error),
    RunFileError(CompassFileError)
}

impl From<std::io::Error> for ProjectError {
    fn from(value: std::io::Error) -> Self {
        ProjectError::ProjectIOError(value)
    }
}

impl From<CompassFileError> for ProjectError {
    fn from(value: CompassFileError) -> Self {
        ProjectError::RunFileError(value)
    }
}

impl std::fmt::Display for ProjectError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ProjectError::ProjectDirError => write!(f, "Project encountered a top level directory error!"),
            ProjectError::RunDirError => write!(f, "Project encountered a run directory error!"),
            ProjectError::ProjectIOError(e) => write!(f, "Project encountered a generic io error: {}", e),
            ProjectError::RunFileError(e) => write!(f, "Project encountered an error with the run files: {}", e)
        }
    }
}

impl std::error::Error for ProjectError {

}

fn is_run_dir(dir: &Path) -> bool {
    dir.file_name()
       .expect("Cannot retrieve directory name at is_run_dir")
       .to_str()
       .expect("Could not convert directory name to str at is_run_dir")
       .contains("run_")
}



#[derive(Debug)]
pub struct Project {
    project_path: PathBuf,
    active_run: Option<ActiveRun>,
    event_queue: Receiver<Event>,
    data_queue: Sender<Bytes>
}

impl Project {

    pub fn new(path: &Path, event: Receiver<Event>, data: Sender<Bytes>) -> Result<Self, ProjectError> {
        if !path.exists() {
            return Err(ProjectError::ProjectDirError);
        }

        let proj = Project { project_path: path.to_path_buf(), active_run: None, event_queue: event, data_queue: data };

        tracing::trace!("Hooked to project directory: {}", proj.project_path.display());
        return Ok(proj);
    }

    pub async fn handle_events(&mut self) -> Result<(), ProjectError> {
        loop {
            tracing::trace!("Started running!");
            match self.event_queue.recv().await {
                Some(event) => {
                    match &event.kind {
                        EventKind::Create(kind) => {
                            match kind {
                                CreateKind::Folder => {
                                    self.handle_create_dir(&event)
                                }
                                _ => {}
                            }
                        },
                        EventKind::Modify(kind) => {
                            match kind {
                                ModifyKind::Any => {
                                    tracing::trace!("Here!");
                                    self.handle_modify_file(&event).await
                                }
                                _ => {}
                            }
                        },
                        _ => { tracing::trace!("Something else!")}
                    }
                },
                None => {
                    tracing::info!("Notify event queue is shutdown");
                    return Ok(())
                }
            };
            tracing::trace!("I made it through!")
        }
    }

    fn handle_create_dir(&mut self, event: &Event) {

        tracing::trace!("Create dir occurred!");
        if event.paths.len() == 0 {
            tracing::trace!("Create with no paths occured!");
            return;
        }
        
        for path in event.paths.iter() {
            if is_run_dir(path) {
                self.active_run = match ActiveRun::new(path) {
                    Ok(ar) => Some(ar),
                    Err(e) => {
                        tracing::error!("Found a dir that looks like a run, but couldn't be inited at Project::handle_create_dir! Error: {}", e);
                        return
                    }
                };
                return;
            }
        }
    }

    async fn handle_modify_file(&mut self, event: &Event) {
        tracing::trace!("Modify file occurred!");

        if event.paths.len() == 0 {
            tracing::trace!("Modify event with no paths occured!");
            return;
        }
        else if self.active_run.is_none() {
            return;
        }

        for path in event.paths.iter() {
            if path.extension().unwrap() == COMPASS_BINARY_EXT {
                let data = self.active_run.as_mut().unwrap().read_data_from_all_files();
                match self.data_queue.send(data).await {
                    Ok(_) => {},
                    Err(e) => tracing::error!("Error on sending data from Project::handle_modify_file: {}", e)
                };
                return;
            }
        }

    }
}

#[derive(Debug)]
struct ActiveRun {
    directory: PathBuf,
    data_files: Vec<CompassFile>
}

impl ActiveRun {

    fn new(new_dir: &Path) -> Result<ActiveRun, ProjectError> {

        if !new_dir.exists() || !new_dir.is_dir() {
            tracing::trace!("Run directory does not exist: {}", new_dir.display());
            return Err(ProjectError::RunDirError);
        }

        let data_directory = new_dir.join("/UNFILTERED/");
        if !data_directory.exists() {
            tracing::trace!("Data directory does not exist: {}", data_directory.display());
            return Err(ProjectError::RunDirError);
        }

        let mut current_run = ActiveRun { directory: data_directory.to_path_buf(), data_files: vec![] };

        for item in new_dir.read_dir()? {
            let filepath = &item?.path();
            if filepath.extension().unwrap() == COMPASS_BINARY_EXT {
                current_run.data_files.push(CompassFile::new(filepath)?);
            }
        }

        tracing::trace!("Reading data in run directory: {}", current_run.directory.display());

        return Ok(current_run);
    }

    fn read_data_from_all_files(&mut self) -> Bytes {
        let mut messages: Vec<Message> = vec![];

        for handle in self.data_files.iter_mut() {
            match handle.read_data() {
                Ok(mess) => messages.push(mess),
                Err(e) => tracing::error!("An error occurred reading file data: {}", e)
            }
        }

        convert_messages_to_bytes(messages)
    }
}