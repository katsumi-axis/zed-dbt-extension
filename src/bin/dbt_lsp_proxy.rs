use serde_json::Value;
use std::env;
use std::io::{self, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::path::PathBuf;
use std::process::{Child, Command, Stdio};
use std::sync::mpsc;
use std::thread;
use std::time::Duration;

const DROP_CLIENT_NOTIFICATIONS: &[&str] = &["workspace/didChangeConfiguration"];
const DBT_BINARY_PATH_ENV: &str = "DBT_BINARY_PATH";

fn log(message: impl AsRef<str>) {
    eprintln!("dbt-lsp-proxy: {}", message.as_ref());
}

struct LspMessage {
    frame: Vec<u8>,
    body: Vec<u8>,
}

enum ProxyEvent {
    DbtSocketEof,
    DbtSocketError(String),
}

fn main() {
    let status = match run() {
        Ok(status) => status,
        Err(error) => {
            eprintln!("dbt-lsp-proxy: {error}");
            1
        }
    };
    std::process::exit(status);
}

fn run() -> Result<i32, String> {
    let project_dir = env::var("DBT_PROJECT_DIR")
        .map(PathBuf::from)
        .unwrap_or(env::current_dir().map_err(|error| format!("failed to read cwd: {error}"))?);

    let extra_args = normalized_extra_args();
    let dbt = env::var_os(DBT_BINARY_PATH_ENV)
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("dbt"));

    log(format!("starting; project_dir={}", project_dir.display()));
    log(format!("using dbt={}", dbt.display()));

    let listener = TcpListener::bind(("127.0.0.1", 0))
        .map_err(|error| format!("failed to bind local socket: {error}"))?;
    let port = listener
        .local_addr()
        .map_err(|error| format!("failed to read local socket address: {error}"))?
        .port();
    log(format!("listening on 127.0.0.1:{port}"));

    let mut command = Command::new(&dbt);
    command
        .arg("lsp")
        .arg("--socket")
        .arg(port.to_string())
        .arg("--project-dir")
        .arg(&project_dir)
        .arg("--quiet")
        .args(extra_args)
        .current_dir(&project_dir)
        .env("DBT_PROJECT_DIR", &project_dir)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::piped());

    log("launching dbt lsp via socket");
    let mut child = command
        .spawn()
        .map_err(|error| format!("failed to launch dbt lsp: {error}"))?;

    listener
        .set_nonblocking(true)
        .map_err(|error| format!("failed to make listener nonblocking: {error}"))?;
    let stream = accept_with_child_watch(&listener, &mut child)?;
    log("dbt lsp socket connected; proxying stdio");

    let (tx, rx) = mpsc::channel();
    spawn_stderr_proxy(&mut child);
    spawn_stdin_proxy(stream.try_clone().map_err(|error| error.to_string())?);
    spawn_socket_stdout_proxy(stream, tx);

    Ok(wait_for_proxy_exit(&mut child, rx))
}

fn normalized_extra_args() -> Vec<String> {
    let mut args: Vec<String> = env::args().skip(1).collect();
    if args.first().is_some_and(|arg| arg == "lsp") {
        args.remove(0);
    }
    args
}

fn accept_with_child_watch(listener: &TcpListener, child: &mut Child) -> Result<TcpStream, String> {
    for _ in 0..300 {
        if let Some(status) = child
            .try_wait()
            .map_err(|error| format!("failed to poll dbt lsp: {error}"))?
        {
            return Err(format!("dbt lsp exited before socket connection: {status}"));
        }

        match listener.accept() {
            Ok((stream, _)) => return Ok(stream),
            Err(error) if error.kind() == io::ErrorKind::WouldBlock => {
                thread::sleep(Duration::from_millis(100));
            }
            Err(error) => return Err(format!("failed to accept dbt lsp socket: {error}")),
        }
    }

    let _ = child.kill();
    log("timed out waiting for dbt lsp socket");
    Err("timed out waiting for dbt lsp socket".into())
}

fn spawn_stderr_proxy(child: &mut Child) {
    let Some(mut stderr) = child.stderr.take() else {
        return;
    };

    thread::spawn(move || {
        let mut buffer = [0; 65536];
        loop {
            match stderr.read(&mut buffer) {
                Ok(0) => {
                    log("dbt stderr reached EOF");
                    break;
                }
                Ok(read) => {
                    let _ = io::stderr().write_all(&buffer[..read]);
                    let _ = io::stderr().flush();
                }
                Err(error) => {
                    log(format!("dbt stderr read failed: {error}"));
                    break;
                }
            }
        }
    });
}

fn spawn_stdin_proxy(mut stream: TcpStream) {
    thread::spawn(move || {
        let mut stdin = io::stdin().lock();
        let mut buffer = [0; 65536];
        let mut lsp_buffer = Vec::new();

        loop {
            match stdin.read(&mut buffer) {
                Ok(0) => {
                    log("zed stdin reached EOF; keeping dbt socket open");
                    break;
                }
                Ok(read) => {
                    lsp_buffer.extend_from_slice(&buffer[..read]);
                    let (messages, remaining) = read_lsp_messages(lsp_buffer);
                    lsp_buffer = remaining;

                    for message in messages {
                        if should_drop_client_message(&message.body) {
                            continue;
                        }

                        if let Err(error) = stream.write_all(&message.frame) {
                            log(format!("dbt socket write failed: {error}"));
                            return;
                        }
                    }
                }
                Err(error) => {
                    log(format!("zed stdin read failed: {error}"));
                    break;
                }
            }
        }
    });
}

fn spawn_socket_stdout_proxy(mut stream: TcpStream, tx: mpsc::Sender<ProxyEvent>) {
    thread::spawn(move || {
        let mut stdout = io::stdout().lock();
        let mut buffer = [0; 65536];

        loop {
            match stream.read(&mut buffer) {
                Ok(0) => {
                    log("dbt socket reached EOF");
                    let _ = tx.send(ProxyEvent::DbtSocketEof);
                    break;
                }
                Ok(read) => {
                    if let Err(error) = stdout.write_all(&buffer[..read]) {
                        let message = format!("zed stdout write failed: {error}");
                        log(&message);
                        let _ = tx.send(ProxyEvent::DbtSocketError(message));
                        break;
                    }
                    let _ = stdout.flush();
                }
                Err(error) => {
                    if error.kind() == io::ErrorKind::WouldBlock {
                        thread::sleep(Duration::from_millis(10));
                        continue;
                    }

                    let message = format!("dbt socket read failed: {error}");
                    log(&message);
                    let _ = tx.send(ProxyEvent::DbtSocketError(message));
                    break;
                }
            }
        }
    });
}

fn wait_for_proxy_exit(child: &mut Child, rx: mpsc::Receiver<ProxyEvent>) -> i32 {
    loop {
        if let Some(status) = child.try_wait().ok().flatten() {
            log(format!("dbt lsp exited with status={status}"));
            return status.code().unwrap_or(1);
        }

        match rx.recv_timeout(Duration::from_millis(100)) {
            Ok(ProxyEvent::DbtSocketEof) => {
                if let Some(status) = child.try_wait().ok().flatten() {
                    log(format!("dbt lsp exited with status={status}"));
                    return status.code().unwrap_or(0);
                }
                log("dbt socket closed before process exit; killing dbt lsp");
                return kill_child(child);
            }
            Ok(ProxyEvent::DbtSocketError(error)) => {
                log(format!("proxy stopping after socket error: {error}"));
                return kill_child(child);
            }
            Err(mpsc::RecvTimeoutError::Timeout) => {}
            Err(mpsc::RecvTimeoutError::Disconnected) => {
                log("proxy event channel disconnected");
                return kill_child(child);
            }
        }
    }
}

fn kill_child(child: &mut Child) -> i32 {
    let _ = child.kill();
    let status = child.wait().ok();
    log(format!("dbt lsp exited after kill with status={status:?}"));
    1
}

fn read_lsp_messages(mut buffer: Vec<u8>) -> (Vec<LspMessage>, Vec<u8>) {
    let mut messages = Vec::new();

    loop {
        let Some(header_end) = find_subsequence(&buffer, b"\r\n\r\n") else {
            return (messages, buffer);
        };

        let headers = String::from_utf8_lossy(&buffer[..header_end]);
        let mut content_length = None;
        for header in headers.split("\r\n") {
            let Some((name, value)) = header.split_once(':') else {
                continue;
            };
            if name.eq_ignore_ascii_case("content-length") {
                content_length = value.trim().parse::<usize>().ok();
            }
        }

        let Some(content_length) = content_length else {
            messages.push(LspMessage {
                frame: buffer,
                body: Vec::new(),
            });
            return (messages, Vec::new());
        };

        let body_start = header_end + 4;
        let body_end = body_start + content_length;
        if buffer.len() < body_end {
            return (messages, buffer);
        }

        let remaining = buffer.split_off(body_end);
        let body = buffer[body_start..body_end].to_vec();
        messages.push(LspMessage {
            frame: buffer,
            body,
        });
        buffer = remaining;
    }
}

fn should_drop_client_message(body: &[u8]) -> bool {
    let Ok(value) = serde_json::from_slice::<Value>(body) else {
        return false;
    };

    let Some(method) = value.get("method").and_then(Value::as_str) else {
        return false;
    };

    if DROP_CLIENT_NOTIFICATIONS.contains(&method) {
        log(format!(
            "dropped client notification unsupported by dbt lsp: {method}"
        ));
        return true;
    }

    false
}

fn find_subsequence(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    haystack
        .windows(needle.len())
        .position(|window| window == needle)
}
