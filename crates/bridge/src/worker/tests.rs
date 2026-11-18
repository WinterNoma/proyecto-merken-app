use super::*;
use std::sync::atomic::{AtomicUsize, Ordering};

#[test]
fn queue_is_bounded_and_close_drains_accepted_jobs() -> Result<(), Box<dyn std::error::Error>> {
    let directory = tempfile::tempdir()?;
    let worker = Worker::open(
        directory.path().to_owned(),
        "test".into(),
        Box::new(facade::system_clock),
    )?;
    let completed = std::sync::Arc::new(AtomicUsize::new(0));
    let (started, running) = mpsc::sync_channel(0);
    let (release, wait) = mpsc::sync_channel::<()>(0);

    worker.enqueue(Box::new(move |_| {
        let _ = started.send(());
        let _ = wait.recv();
    }))?;
    running.recv()?;

    for _ in 0..QUEUE_CAPACITY {
        let completed = completed.clone();
        worker.enqueue(Box::new(move |_| {
            completed.fetch_add(1, Ordering::Relaxed);
        }))?;
    }
    let overflow = worker.enqueue(Box::new(|_| {}));
    drop(release);
    let error = overflow.err().ok_or("Expected a full queue")?;
    assert_eq!(error.code, "busy");

    worker.close()?;
    assert_eq!(completed.load(Ordering::Relaxed), QUEUE_CAPACITY);
    worker.close()?;
    let error = worker.run(|_| Ok(())).err().ok_or("Expected closure")?;
    assert_eq!(error.code, "closed");
    Ok(())
}

#[test]
fn startup_returns_the_facade_error() -> Result<(), Box<dyn std::error::Error>> {
    let error = Worker::open(
        "relative".into(),
        "test".into(),
        Box::new(facade::system_clock),
    )
    .err()
    .ok_or("Expected an invalid application directory")?;

    assert_eq!(error.code, "invalid_value");
    assert_eq!(error.field.as_deref(), Some("private_dir"));
    Ok(())
}
