// Semantics-layer spike (issue #21): what does rPGP give for free on the
// OpenPGP layer-4 semantics (revocation / expiry / key-flag), and what must
// kleio-crypto hand-build? Runs against real gpg-generated keys.
use pgp::composed::{Deserializable, Message, MessageBuilder, SignedPublicKey, SignedSecretKey};
use pgp::crypto::sym::SymmetricKeyAlgorithm;
use pgp::packet::KeyFlags;
use pgp::types::KeyDetails;
use rand::thread_rng;
use std::path::Path;
use std::process::Command;
use std::time::SystemTime;

// ---- gpg driver (synthetic fixtures, generated at runtime, never committed) ----

fn gpg(home: &Path, args: &[&str]) -> String {
    let out = Command::new("gpg")
        .arg("--batch")
        .arg("--homedir")
        .arg(home)
        .args(args)
        .output()
        .expect("failed to spawn gpg");
    if !out.status.success() {
        panic!("gpg {:?} failed: {}", args, String::from_utf8_lossy(&out.stderr));
    }
    String::from_utf8_lossy(&out.stdout).into_owned()
}

fn fpr_of(colons: &str) -> String {
    colons
        .lines()
        .find_map(|l| l.strip_prefix("fpr:::::::::"))
        .expect("no fpr in gpg colons output")
        .trim_end_matches(':')
        .to_string()
}

fn gen_key(home: &Path, name: &str, email: &str) -> String {
    gpg(
        home,
        &[
            "--passphrase", "",
            "--quick-gen-key",
            &format!("{name} <{email}>"),
            "default", "default", "never",
        ],
    );
    fpr_of(&gpg(home, &["--with-colons", "--list-keys", email]))
}

struct Fixture {
    label: &'static str,
    pub_armored: String,
    sec_armored: Option<String>,
}

fn revoke_key(home: &Path, fpr: &str) {
    use std::io::Write;
    use std::process::Stdio;

    let mut gen = Command::new("gpg")
        .arg("--homedir")
        .arg(home)
        .arg("--no-tty")
        .arg("--command-fd").arg("0")
        .arg("--gen-revoke").arg(fpr)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .expect("spawn gpg");
    gen.stdin.as_mut().unwrap().write_all(b"y\n0\n\ny\n").expect("write answers");
    let out = gen.wait_with_output().expect("gen-revoke");
    if !out.status.success() {
        panic!("gen-revoke failed: {}", String::from_utf8_lossy(&out.stderr));
    }
    let cert = String::from_utf8_lossy(&out.stdout).into_owned();

    let mut imp = Command::new("gpg")
        .arg("--batch")
        .arg("--homedir")
        .arg(home)
        .arg("--import")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .expect("spawn gpg");
    imp.stdin.as_mut().unwrap().write_all(cert.as_bytes()).expect("write cert");
    let imp = imp.wait_with_output().expect("import");
    if !imp.status.success() {
        panic!("revocation import failed: {}", String::from_utf8_lossy(&imp.stderr));
    }
}

fn make_fixtures(home: &Path) -> Vec<Fixture> {
    let fresh = gen_key(home, "Spike Fresh", "fresh@kleio.test");
    let expired = gen_key(home, "Spike Expired", "expired@kleio.test");
    let an_hour_ago = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap()
        .as_secs()
        - 3600;
    gpg(home, &["--yes", "--quick-set-expire", &expired, &an_hour_ago.to_string()]);
    let revoked = gen_key(home, "Spike Revoked", "revoked@kleio.test");
    revoke_key(home, &revoked);

    [("fresh", fresh), ("expired", expired), ("revoked", revoked)]
        .into_iter()
        .map(|(label, fpr)| Fixture {
            label,
            pub_armored: gpg(home, &["--armor", "--export", &fpr]),
            sec_armored: (label == "fresh").then(|| gpg(home, &["--armor", "--export-secret-keys", &fpr])),
        })
        .collect()
}

// ---- hand-built layer-4 decisions (the part rPGP does NOT provide) ----

/// Self-signatures: direct key signatures plus every user-id self-signature.
/// rPGP groups them for free; picking the *authoritative* one is ours.
fn self_signatures(key: &SignedPublicKey) -> impl Iterator<Item = &pgp::packet::Signature> {
    key.details
        .direct_signatures
        .iter()
        .chain(key.details.users.iter().flat_map(|u| u.signatures.iter()))
}

/// rPGP parses a relative Duration for free; adding creation time and
/// comparing against "now" is hand-built (RFC 9580 §5.2.3.9).
fn expires_at(key: &SignedPublicKey) -> Option<SystemTime> {
    let ttl = self_signatures(key).find_map(|s| s.key_expiration_time())?;
    let created: SystemTime = key.primary_key.created_at().into();
    Some(created + std::time::Duration::from(ttl))
}

fn is_expired(key: &SignedPublicKey) -> bool {
    expires_at(key).map(|t| t <= SystemTime::now()).unwrap_or(false)
}

/// rPGP parses revocation signatures (grouped, reason codes attached) but
/// has no "is revoked" answer; verifying them and deciding is hand-built.
fn is_revoked(key: &SignedPublicKey) -> bool {
    key.details
        .revocation_signatures
        .iter()
        .any(|sig| sig.verify_key(&key.primary_key).is_ok())
}

/// rPGP parses key-flag bytes into a bitfield for free; mapping flags to
/// allowed operations is policy, hand-built.
fn key_flags(key: &SignedPublicKey) -> KeyFlags {
    self_signatures(key).find_map(|s| Some(s.key_flags())).unwrap_or_default()
}

fn can_encrypt(key: &SignedPublicKey) -> bool {
    if is_revoked(key) || is_expired(key) {
        return false;
    }
    let f = key_flags(key);
    f.encrypt_comms() || f.encrypt_storage()
}

// ---- spike ----

fn main() {
    let home = std::env::temp_dir().join(format!("kleio-spike-gnupg-{}", std::process::id()));
    std::fs::create_dir_all(&home).expect("temp gnupghome");
    println!("gnupghome: {}\n", home.display());

    let fixtures = make_fixtures(&home);
    let hand_built_lines = 14; // the four helpers above, plus policy in can_encrypt

    for fx in &fixtures {
        let (key, _) = SignedPublicKey::from_reader_single(fx.pub_armored.as_bytes())
            .expect("parse armor");
        key.verify_bindings().expect("verify bindings");

        let userid = key
            .details
            .users
            .iter()
            .find_map(|u| u.id.as_str())
            .unwrap_or("<none>");

        let exp = expires_at(&key);
        let expired = is_expired(&key);
        let revoked = is_revoked(&key);
        let flags = key_flags(&key);

        println!("== {:<8} {userid}", fx.label);
        println!(
            "   parsed (free):      algorithm={:?}, self-sigs={}",
            key.primary_key.algorithm(),
            self_signatures(&key).count()
        );
        println!(
            "   parsed (free):      key_expiration_time={:?}",
            self_signatures(&key).find_map(|s| s.key_expiration_time())
        );
        println!(
            "   parsed (free):      key_flags (certify={} sign={} enc_comms={} enc_storage={} auth={})",
            flags.certify(),
            flags.sign(),
            flags.encrypt_comms(),
            flags.encrypt_storage(),
            flags.authentication()
        );
        println!(
            "   parsed (free):      revocation_signatures={}",
            key.details.revocation_signatures.len()
        );
        println!("   hand-built:         expires_at={exp:?} -> expired={expired}");
        println!("   hand-built:         revoked={revoked}");
        println!("   hand-built:         can_encrypt={}", can_encrypt(&key));
        println!();
    }

    // Pass-store-style round trip on the fresh key (SEIPD v1 + AES-256: the
    // universally gpg-compatible choice per research ticket #20).
    let fresh = &fixtures[0];
    let (cert, _) = SignedPublicKey::from_reader_single(fresh.pub_armored.as_bytes()).unwrap();
    let (sec_key, _) =
        SignedSecretKey::from_reader_single(fresh.sec_armored.as_ref().unwrap().as_bytes()).unwrap();
    sec_key.verify_bindings().expect("verify bindings");

    // Encryption-subkey selection is itself a layer-4 decision: "pick a subkey
    // that can encrypt" must also consider the subkey's own flags, expiry and
    // revocation — exactly what rPGP does not do (see pgp 0.20 example).
    let enc_subkey = cert
        .public_subkeys
        .iter()
        .find(|sk| sk.key.algorithm().can_encrypt())
        .expect("gpg default key has an encryption subkey");
    println!(
        "== pass-store-style round trip (fresh key, subkey {})",
        enc_subkey.key.fingerprint()
    );

    let entry: &[u8] = b"correct horse battery staple\n";
    let mut builder = MessageBuilder::from_bytes("github.com/example", entry.to_vec())
        .seipd_v1(thread_rng(), SymmetricKeyAlgorithm::AES256);
    builder.encrypt_to_key(thread_rng(), enc_subkey).expect("encrypt to subkey");
    let armored = builder
        .to_armored_string(thread_rng(), Default::default())
        .expect("armor");
    println!("   ciphertext bytes: {}", armored.len());

    let (decrypted_msg, _) = Message::from_armor(armored.as_bytes()).expect("parse message");
    let decrypted = decrypted_msg
        .decrypt(&"".into(), &sec_key)
        .expect("decrypt")
        .as_data_vec()
        .expect("read plaintext");
    println!("   plaintext matches: {}", decrypted == entry);
    println!();

    println!("== measurement");
    println!("   rPGP calls (parse + crypto):  6 per key + 5 for the round trip");
    println!("   hand-built semantics lines:   {hand_built_lines} (expiry check, revocation check, key-flag policy)");
    println!(
        "   every layer-4 decision (expired?, revoked?, allowed-ops?, which subkey?) \
         is hand-built; rPGP only parses the underlying data."
    );
}
