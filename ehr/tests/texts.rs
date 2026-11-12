//! The text sender's real request (ehr/src/texts.rs), against a local listener that reads it the
//! way Twilio (or the fake Twilio) would: the path, the Basic auth, the form.

use deetshealth_core::Shielded;
use deetshealth_ehr::{
    texts::{TwilioCodes, TwilioConfig},
    verify::Codes,
};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::TcpListener,
};
use zeroize::Zeroizing;

/// Reads one HTTP request, answers with `status`, returns the request's text.
async fn answer_once(listener: TcpListener, status: &'static str) -> String {
    let (mut sock, _) = listener.accept().await.unwrap();
    let mut buf = Vec::new();
    let mut chunk = [0u8; 4096];
    loop {
        let n = sock.read(&mut chunk).await.unwrap();
        buf.extend_from_slice(&chunk[..n]);
        let text = String::from_utf8_lossy(&buf);
        if let Some(head_end) = text.find("\r\n\r\n") {
            let len: usize = text[..head_end]
                .lines()
                .find_map(|l| l.to_ascii_lowercase().strip_prefix("content-length:").map(|v| v.trim().parse().unwrap()))
                .unwrap_or(0);
            if buf.len() >= head_end + 4 + len {
                break;
            }
        }
        if n == 0 {
            break;
        }
    }
    let reply = format!("HTTP/1.1 {status}\r\ncontent-length: 2\r\nconnection: close\r\n\r\n{{}}");
    sock.write_all(reply.as_bytes()).await.unwrap();
    String::from_utf8(buf).unwrap()
}

fn sender(port: u16) -> TwilioCodes {
    let config = TwilioConfig::new(
        &format!("http://127.0.0.1:{port}"),
        format!("AC{}", "a".repeat(32)),
        Zeroizing::new("secret-token".into()),
        format!("MG{}", "b".repeat(32)),
    )
    .unwrap();
    TwilioCodes::new(config).unwrap()
}

#[tokio::test]
async fn the_request_is_twilios_messages_call() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    let server = tokio::spawn(answer_once(listener, "201 Created"));

    let codes = sender(port);
    let sent = codes.send(&Shielded::new("+15555550101".into()), &Shielded::new("482913".into())).await;
    assert!(sent.is_ok());

    let req = server.await.unwrap();
    let path = format!("POST /2010-04-01/Accounts/AC{}/Messages.json HTTP/1.1", "a".repeat(32));
    assert!(req.starts_with(&path), "{}", req.lines().next().unwrap_or(""));
    // base64("ACaaa…:secret-token")
    let auth = format!("AC{}:secret-token", "a".repeat(32));
    assert!(req.to_ascii_lowercase().contains("content-type: application/x-www-form-urlencoded"));
    assert!(req.contains(&format!("authorization: Basic {}", b64(auth.as_bytes()))));
    assert!(req.contains("To=%2B15555550101"));
    assert!(req.contains(&format!("MessagingServiceSid=MG{}", "b".repeat(32))));
    assert!(req.contains("482913"), "the code is in the body");
}

#[tokio::test]
async fn a_refusal_is_down_and_says_nothing() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    let server = tokio::spawn(answer_once(listener, "400 Bad Request"));
    let codes = sender(port);
    let sent = codes.send(&Shielded::new("+15555550101".into()), &Shielded::new("482913".into())).await;
    assert!(sent.is_err());
    server.await.unwrap();
}

#[tokio::test]
async fn each_code_is_new() {
    let codes = sender(1);
    let a = codes.issue();
    let b = codes.issue();
    let c = codes.issue();
    assert!(!(a == b && b == c), "three equal random codes");
}

fn b64(bytes: &[u8]) -> String {
    const T: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::new();
    for chunk in bytes.chunks(3) {
        let n = chunk.iter().enumerate().fold(0u32, |n, (i, &b)| n | (b as u32) << (16 - 8 * i));
        for i in 0..4 {
            if i <= chunk.len() {
                out.push(T[(n >> (18 - 6 * i) & 63) as usize] as char);
            } else {
                out.push('=');
            }
        }
    }
    out
}
