use crate::api::ApiError;
use pulsys_api::{self as facade, App, Clock};
use std::{
    path::PathBuf,
    sync::{
        Mutex,
        mpsc::{self, Receiver, SyncSender, TrySendError},
    },
    thread::{self, JoinHandle},
};

const QUEUE_CAPACITY: usize = 16;
type Job = Box<dyn FnOnce(&mut App) + Send>;

pub(crate) struct Worker {
    sender: Mutex<Option<SyncSender<Job>>>,
    shutdown: Mutex<Shutdown>,
}

struct Shutdown {
    thread: Option<JoinHandle<Result<(), ApiError>>>,
    result: Result<(), ApiError>,
}

impl Worker {
    pub(crate) fn open(
        private_dir: PathBuf,
        app_version: String,
        clock: Clock,
    ) -> Result<Self, ApiError> {
        let (sender, jobs) = mpsc::sync_channel(QUEUE_CAPACITY);
        let (ready, startup) = mpsc::sync_channel(1);
        let thread = thread::Builder::new()
            .name("pulsys-store".into())
            .spawn(move || run_worker(jobs, ready, private_dir, app_version, clock))
            .map_err(|_| worker_error("The store worker could not be started"))?;

        match startup.recv() {
            Ok(Ok(())) => Ok(Self {
                sender: Mutex::new(Some(sender)),
                shutdown: Mutex::new(Shutdown {
                    thread: Some(thread),
                    result: Ok(()),
                }),
            }),
            Ok(Err(error)) => {
                join_worker(thread)?;
                Err(error)
            }
            Err(_) => {
                join_worker(thread)?;
                Err(closed())
            }
        }
    }

    pub(crate) fn run<T: Send + 'static>(
        &self,
        operation: impl FnOnce(&mut App) -> Result<T, ApiError> + Send + 'static,
    ) -> Result<T, ApiError> {
        let (respond, response) = mpsc::sync_channel(1);
        self.enqueue(Box::new(move |app| {
            let _ = respond.send(operation(app));
        }))?;
        response.recv().map_err(|_| closed())?
    }

    fn enqueue(&self, job: Job) -> Result<(), ApiError> {
        let sender = self.sender.lock().map_err(|_| closed())?;
        sender
            .as_ref()
            .ok_or_else(closed)?
            .try_send(job)
            .map_err(|error| match error {
                TrySendError::Full(_) => ApiError {
                    code: "busy".into(),
                    field: None,
                    message: "The store queue is full".into(),
                },
                TrySendError::Disconnected(_) => closed(),
            })
    }

    pub(crate) fn close(&self) -> Result<(), ApiError> {
        let mut shutdown = self.shutdown.lock().map_err(|_| closed())?;
        drop(self.sender.lock().map_err(|_| closed())?.take());
        if let Some(thread) = shutdown.thread.take() {
            shutdown.result = join_worker(thread);
        }
        shutdown.result.clone()
    }
}

fn run_worker(
    jobs: Receiver<Job>,
    ready: SyncSender<Result<(), ApiError>>,
    private_dir: PathBuf,
    app_version: String,
    clock: Clock,
) -> Result<(), ApiError> {
    let mut app = match facade::open_store(&private_dir, app_version, clock) {
        Ok(app) => app,
        Err(error) => {
            let _ = ready.send(Err(error.into()));
            return Ok(());
        }
    };
    if ready.send(Ok(())).is_ok() {
        for job in jobs {
            job(&mut app);
        }
    }
    facade::close_store(app).map_err(Into::into)
}

fn join_worker(thread: JoinHandle<Result<(), ApiError>>) -> Result<(), ApiError> {
    thread
        .join()
        .map_err(|_| worker_error("The store worker ended unexpectedly"))?
}

fn closed() -> ApiError {
    ApiError {
        code: "closed".into(),
        field: None,
        message: "The store session is closed".into(),
    }
}

fn worker_error(message: &str) -> ApiError {
    ApiError {
        code: "storage_error".into(),
        field: None,
        message: message.into(),
    }
}

#[cfg(test)]
mod tests;
