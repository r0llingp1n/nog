use notify::{RecursiveMode, Watcher};
use std::fs;
use std::process::Command;
use std::sync::mpsc;
use std::time::Duration;

#[test]
fn test_inotify_on_git_dir() {
    // Create a temp git repo
    let tmp = std::env::temp_dir().join("nog_fs_test");
    let _ = fs::remove_dir_all(&tmp);
    fs::create_dir_all(&tmp).unwrap();

    Command::new("git")
        .args(["init", tmp.to_str().unwrap()])
        .output()
        .expect("git init failed");

    // Set up watcher on .git directory (mirrors nog's setup_watcher)
    let git_dir = tmp.join(".git");
    let (tx, rx) = mpsc::channel();

    let mut watcher = notify::recommended_watcher(move |res: Result<notify::Event, notify::Error>| {
        if let Ok(event) = res {
            use notify::EventKind;
            match event.kind {
                EventKind::Modify(_) | EventKind::Create(_) | EventKind::Remove(_) => {
                    let _ = tx.send(event.kind);
                }
                _ => {}
            }
        }
    })
    .expect("failed to create watcher");

    watcher
        .watch(&git_dir, RecursiveMode::Recursive)
        .expect("failed to watch .git dir");

    // Also watch workdir like nog does
    watcher
        .watch(&tmp, RecursiveMode::NonRecursive)
        .expect("failed to watch workdir");

    // Make a change: create a file in the workdir
    fs::write(tmp.join("hello.txt"), "world").unwrap();

    // Wait for an event (2 second timeout)
    let event = rx.recv_timeout(Duration::from_secs(2));
    assert!(
        event.is_ok(),
        "expected fs event within 2s, got timeout. inotify may not be working."
    );
    eprintln!("received event: {:?}", event.unwrap());

    // Also test .git dir changes (simulate a commit-like write)
    fs::write(git_dir.join("test_ref"), "abc123").unwrap();
    let event2 = rx.recv_timeout(Duration::from_secs(2));
    assert!(
        event2.is_ok(),
        "expected fs event on .git dir within 2s"
    );
    eprintln!("received .git event: {:?}", event2.unwrap());

    // Cleanup
    let _ = fs::remove_dir_all(&tmp);
}
