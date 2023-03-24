use tokio::sync::mpsc::{Receiver, Sender};
use bytes::Bytes;
use std::fs::File;
use std::path::{Path, PathBuf};
use std::io::{BufReader, Read};
use notify::event::{Event, EventKind, CreateKind, ModifyKind};

const COMPASS_BINARY_EXT: &str = ".BIN";

#[derive(Debug)]
pub enum ProjectError {
    ProjectDirError,
    RunDirError,
    RunFilesError(std::io::Error)
}

impl From<std::io::Error> for ProjectError {
    fn from(value: std::io::Error) -> Self {
        ProjectError::RunFilesError(value)
    }
}

impl std::fmt::Display for ProjectError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ProjectError::ProjectDirError => write!(f, "Project encountered a top level directory error!"),
            ProjectError::RunDirError => write!(f, "Project encountered a run directory error!"),
            ProjectError::RunFilesError(e) => write!(f, "Project encountered a run file error: {}", e),
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

        println!("Hooked to project directory: {}", proj.project_path.display());
        return Ok(proj);
    }

    pub async fn handle_events(&mut self) -> Result<(), ProjectError> {
        loop {
            println!("Started running!");
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
                                    println!("Here!");
                                    self.handle_modify_file(&event).await
                                }
                                _ => {}
                            }
                        },
                        _ => { println!("Something else!")}
                    }
                },
                None => {
                    println!("Notify event queue is shutdown");
                    return Ok(())
                }
            };
            println!("I made it through!")
        }
    }

    fn handle_create_dir(&mut self, event: &Event) {

        println!("Create dir occurred!");
        if event.paths.len() == 0 {
            println!("Create with no paths occured!");
            return;
        }
        
        for path in event.paths.iter() {
            if is_run_dir(path) {
                self.active_run = match ActiveRun::new(path) {
                    Ok(ar) => Some(ar),
                    Err(e) => {
                        println!("Found a dir that looks like a run, but couldn't be inited at Project::handle_create_dir! Error: {}", e);
                        return
                    }
                };
                return;
            }
        }
    }

    async fn handle_modify_file(&mut self, event: &Event) {
        println!("Modify file occurred!");

        if event.paths.len() == 0 {
            println!("Modify event with no paths occured!");
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
                    Err(e) => println!("Error on sending data from Project::handle_modify_file: {}", e)
                };
                return;
            }
        }

    }
}

#[derive(Debug)]
struct ActiveRun {
    directory: PathBuf,
    data_files: Vec<BufReader<File>>
}

impl ActiveRun {

    fn new(new_dir: &Path) -> Result<ActiveRun, ProjectError> {

        if !new_dir.exists() || !new_dir.is_dir() {
            println!("Run directory does not exist: {}", new_dir.display());
            return Err(ProjectError::RunDirError);
        }

        let data_directory = new_dir.join("/UNFILTERED/");
        if !data_directory.exists() {
            println!("Data directory does not exist: {}", data_directory.display());
            return Err(ProjectError::RunDirError);
        }

        let mut current_run = ActiveRun { directory: data_directory.to_path_buf(), data_files: vec![] };

        for item in new_dir.read_dir()? {
            let filepath = &item?.path();
            if filepath.extension().unwrap() == COMPASS_BINARY_EXT {
                let file = File::open(filepath)?;
                current_run.data_files.push(BufReader::new(file));
            }
        }

        println!("Reading data in run directory: {}", current_run.directory.display());

        return Ok(current_run);
    }

    fn read_data_from_all_files(&mut self) -> Bytes {
        let mut data: Vec<u8> = vec![];

        for handle in self.data_files.iter_mut() {
            match handle.read_to_end(&mut data) {
                Ok(_size) => {},
                Err(e) => {
                    println!("Recieved an error while reading some data from file: {}", e);
                }
            }
        }

        Bytes::from(data)
    }
}