use git2::{build::RepoBuilder, Cred, FetchOptions, PushOptions, RemoteCallbacks, Signature};
use std::path::Path;

// Throwaway spike for wayfinder ticket "git2 fetch + push + clone over SSH in-process
// smoke test" (map #28). Proves git2/libgit2 drives SSH via in-process libssh2 — no
// `ssh` subprocess. See README.md for the verdict and the one interop caveat.

const KEY_PUB: &str = "/tmp/kleio-smoke/id_ed25519.pub";
const KEY_PRIV: &str = "/tmp/kleio-smoke/id_ed25519";

fn callbacks() -> RemoteCallbacks<'static> {
    let mut cb = RemoteCallbacks::new();
    // Trust the local sshd host key (throwaway spike: trust-on-first-use is fine here).
    cb.certificate_check(|_cert, _host| Ok(git2::CertificateCheckStatus::CertificateOk));
    cb.credentials(|_url, username_from_url, allowed| {
        let user = username_from_url
            .map(|s| s.to_string())
            .or_else(|| std::env::var("USER").ok())
            .unwrap_or_else(|| "oliverbrotchie".to_string());
        if allowed.contains(git2::CredentialType::USERNAME) {
            return git2::Cred::username(&user);
        }
        if allowed.contains(git2::CredentialType::SSH_KEY) {
            return git2::Cred::ssh_key(&user, Some(Path::new(KEY_PUB)), Path::new(KEY_PRIV), None);
        }
        Err(git2::Error::from_str("unsupported credential type"))
    });
    cb
}

fn main() {
    let url = "ssh://127.0.0.1:2222/tmp/kleio-smoke/remote.git";

    // 1. clone over SSH (in-process: no `ssh` binary is used)
    let work = "/tmp/kleio-smoke/work";
    let _ = std::fs::remove_dir_all(work);
    let mut fo = FetchOptions::new();
    fo.remote_callbacks(callbacks());
    let mut builder = RepoBuilder::new();
    builder.fetch_options(fo);
    let repo = builder.clone(url, Path::new(work)).expect("clone over ssh");
    println!("CLONE OK");

    // 2. write a file, commit
    std::fs::write(format!("{work}/hello.txt"), "hello\n").unwrap();
    let mut index = repo.index().unwrap();
    index.add_path(Path::new("hello.txt")).unwrap();
    let tree = repo.find_tree(index.write_tree().unwrap()).unwrap();
    let sig = Signature::now("Smoke", "smoke@example.com").unwrap();
    let parent = repo.head().unwrap().peel_to_commit().unwrap();
    repo.commit(Some("HEAD"), &sig, &sig, "smoke commit", &tree, &[&parent])
        .unwrap();
    println!("COMMIT OK");

    // 3. push over SSH
    let mut remote = repo.find_remote("origin").unwrap();
    let mut po = PushOptions::new();
    po.remote_callbacks(callbacks());
    remote
        .push(&["refs/heads/main:refs/heads/main"], Some(&mut po))
        .expect("push over ssh");
    println!("PUSH OK");

    // 4. fresh clone + fetch to verify the pushed commit arrived
    let work2 = "/tmp/kleio-smoke/work2";
    let _ = std::fs::remove_dir_all(work2);
    let mut fo2 = FetchOptions::new();
    fo2.remote_callbacks(callbacks());
    let mut b2 = RepoBuilder::new();
    b2.fetch_options(fo2);
    let repo2 = b2.clone(url, Path::new(work2)).expect("clone2 over ssh");
    let head2 = repo2.head().unwrap().peel_to_commit().unwrap();
    let has_hello = std::path::Path::new(&format!("{work2}/hello.txt")).exists();
    println!("VERIFY head={} hello_present={has_hello}", &head2.id().to_string()[..8]);
    assert!(has_hello, "pushed commit not present in fresh clone");

    println!("SMOKE PASS");
}
