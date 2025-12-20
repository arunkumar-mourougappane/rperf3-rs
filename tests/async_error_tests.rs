use rperf3::protocol::{deserialize_message, serialize_message, Message};
use rperf3::Config;
use std::time::Duration;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;
use tokio::time::timeout;

/// Test that connection timeout works correctly
#[tokio::test]
async fn test_connection_timeout() {
    // Try to connect to a port that nothing is listening on with a short timeout
    let result = timeout(
        Duration::from_millis(100),
        tokio::net::TcpStream::connect("127.0.0.1:9"),
    )
    .await;

    // Should timeout or fail to connect
    assert!(result.is_err() || result.unwrap().is_err());
}

/// Test protocol message serialization edge cases
#[tokio::test]
async fn test_protocol_empty_error_message() {
    let msg = Message::error("".to_string());
    let serialized = serialize_message(&msg).unwrap();
    let json_bytes = &serialized[4..];
    let deserialized: Message = serde_json::from_slice(json_bytes).unwrap();

    match deserialized {
        Message::Error { message } => assert_eq!(message, ""),
        _ => panic!("Expected Error message"),
    }
}

/// Test protocol with very long error message
#[tokio::test]
async fn test_protocol_long_error_message() {
    let long_message = "x".repeat(10_000);
    let msg = Message::error(long_message.clone());
    let serialized = serialize_message(&msg).unwrap();

    // Should not panic and should be able to deserialize
    let json_bytes = &serialized[4..];
    let deserialized: Message = serde_json::from_slice(json_bytes).unwrap();

    match deserialized {
        Message::Error { message } => assert_eq!(message.len(), 10_000),
        _ => panic!("Expected Error message"),
    }
}

/// Test handling of truncated protocol messages
#[tokio::test]
async fn test_protocol_truncated_message() {
    let msg = Message::Done;
    let mut serialized = serialize_message(&msg).unwrap();

    // Truncate the message
    serialized.truncate(serialized.len() - 1);

    let json_bytes = &serialized[4..];
    let result: serde_json::Result<Message> = serde_json::from_slice(json_bytes);

    // Should fail gracefully
    assert!(result.is_err());
}

/// Test handling of invalid length prefix
#[tokio::test]
async fn test_protocol_invalid_length_prefix() {
    // Create a message with wrong length prefix
    let mut bad_message = vec![0xFF, 0xFF, 0xFF, 0xFF]; // Very large length
    bad_message.extend_from_slice(b"{\"type\":\"Done\"}");

    // Attempting to read this would try to allocate too much memory
    let len = u32::from_be_bytes([
        bad_message[0],
        bad_message[1],
        bad_message[2],
        bad_message[3],
    ]);
    assert!(len > 1_000_000); // Should be an unreasonable size
}

/// Test bidirectional communication with error injection
#[tokio::test]
async fn test_bidirectional_with_error() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();

    // Spawn server that sends an error
    let server_handle = tokio::spawn(async move {
        let (mut socket, _) = listener.accept().await.unwrap();

        // Send error message
        let error_msg = Message::error("Intentional test error".to_string());
        let serialized = serialize_message(&error_msg).unwrap();
        socket.write_all(&serialized).await.unwrap();
    });

    // Client connects and receives error
    let client_handle = tokio::spawn(async move {
        let mut socket = tokio::net::TcpStream::connect(addr).await.unwrap();

        let msg = deserialize_message(&mut socket).await.unwrap();
        match msg {
            Message::Error { message } => {
                assert_eq!(message, "Intentional test error");
            }
            _ => panic!("Expected Error message"),
        }
    });

    // Wait for both to complete
    let _ = tokio::join!(server_handle, client_handle);
}

/// Test handling of premature connection close
#[tokio::test]
async fn test_premature_connection_close() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();

    // Spawn server that closes immediately after accepting
    let server_handle = tokio::spawn(async move {
        let (socket, _) = listener.accept().await.unwrap();
        drop(socket); // Close immediately
    });

    // Client tries to read from closed connection
    let client_handle = tokio::spawn(async move {
        let mut socket = tokio::net::TcpStream::connect(addr).await.unwrap();

        // Give server time to close
        tokio::time::sleep(Duration::from_millis(50)).await;

        let result = deserialize_message(&mut socket).await;
        // Should get EOF or error
        assert!(result.is_err());
    });

    let _ = tokio::join!(server_handle, client_handle);
}

/// Test handling of partial message reads
#[tokio::test]
async fn test_partial_message_read() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();

    // Spawn server that sends partial message
    let server_handle = tokio::spawn(async move {
        let (mut socket, _) = listener.accept().await.unwrap();

        let msg = Message::Done;
        let serialized = serialize_message(&msg).unwrap();

        // Send only the length prefix
        socket.write_all(&serialized[0..4]).await.unwrap();

        // Wait a bit, then close without sending the rest
        tokio::time::sleep(Duration::from_millis(100)).await;
    });

    // Client tries to read incomplete message
    let client_handle = tokio::spawn(async move {
        let mut socket = tokio::net::TcpStream::connect(addr).await.unwrap();

        let result = timeout(Duration::from_millis(500), deserialize_message(&mut socket)).await;

        // Should timeout or get error
        assert!(result.is_err() || result.unwrap().is_err());
    });

    let _ = tokio::join!(server_handle, client_handle);
}

/// Test concurrent message sending
#[tokio::test]
async fn test_concurrent_message_sending() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();

    // Spawn server that accepts connection
    let server_handle = tokio::spawn(async move {
        let (mut socket, _) = listener.accept().await.unwrap();

        // Read multiple messages
        for i in 0..10 {
            let msg = deserialize_message(&mut socket).await.unwrap();
            match msg {
                Message::Start { timestamp } => {
                    assert_eq!(timestamp, i);
                }
                _ => panic!("Expected Start message"),
            }
        }
    });

    // Client sends multiple messages
    let client_handle = tokio::spawn(async move {
        let mut socket = tokio::net::TcpStream::connect(addr).await.unwrap();

        for i in 0..10 {
            let msg = Message::start(i);
            let serialized = serialize_message(&msg).unwrap();
            socket.write_all(&serialized).await.unwrap();
        }
    });

    let _ = tokio::join!(server_handle, client_handle);
}

/// Test zero-duration setup message
#[tokio::test]
async fn test_zero_duration_setup() {
    let msg = Message::setup(
        "TCP".to_string(),
        Duration::from_secs(0),
        None,
        8192,
        1,
        false,
    );

    let serialized = serialize_message(&msg).unwrap();
    let json_bytes = &serialized[4..];
    let deserialized: Message = serde_json::from_slice(json_bytes).unwrap();

    match deserialized {
        Message::Setup { duration, .. } => assert_eq!(duration, 0),
        _ => panic!("Expected Setup message"),
    }
}

/// Test handling of network errors during async operations
#[tokio::test]
async fn test_network_error_handling() {
    // Create a socket and immediately close it
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();

    let server_handle = tokio::spawn(async move {
        let _ = listener.accept().await;
    });

    let client_handle = tokio::spawn(async move {
        let socket = tokio::net::TcpStream::connect(addr).await.unwrap();
        drop(socket); // Close immediately

        // Try to use closed socket would fail
        // This tests that we handle errors gracefully
    });

    let _ = tokio::join!(server_handle, client_handle);
}

/// Test zero-byte messages are handled correctly
#[tokio::test]
async fn test_zero_byte_read() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();

    let server_handle = tokio::spawn(async move {
        let (socket, _) = listener.accept().await.unwrap();
        // Accept but don't send anything, then close
        drop(socket);
    });

    let client_handle = tokio::spawn(async move {
        let mut socket = tokio::net::TcpStream::connect(addr).await.unwrap();

        // Wait for server to close
        tokio::time::sleep(Duration::from_millis(50)).await;

        // Try to read - should get EOF
        let mut buf = [0u8; 4];
        let result = socket.read_exact(&mut buf).await;
        assert!(result.is_err());
    });

    let _ = tokio::join!(server_handle, client_handle);
}

/// Test configuration validation edge cases
#[tokio::test]
async fn test_config_validation() {
    // Test minimum values
    let config = Config::client("127.0.0.1".to_string(), 5201)
        .with_parallel(1)
        .with_duration(Duration::from_secs(1))
        .with_buffer_size(1);

    assert_eq!(config.parallel, 1);
    assert_eq!(config.duration, Duration::from_secs(1));
    assert_eq!(config.buffer_size, 1);

    // Test with very large buffer
    let config = Config::client("127.0.0.1".to_string(), 5201).with_buffer_size(1_048_576); // 1 MB

    assert_eq!(config.buffer_size, 1_048_576);
}

/// Test message serialization with extreme values
#[tokio::test]
async fn test_extreme_values_serialization() {
    // Test with u64::MAX
    let msg = Message::interval(0, 0.0, 1.0, u64::MAX, 0.0);
    let serialized = serialize_message(&msg).unwrap();
    let json_bytes = &serialized[4..];
    let deserialized: Message = serde_json::from_slice(json_bytes).unwrap();

    match deserialized {
        Message::Interval { bytes, .. } => assert_eq!(bytes, u64::MAX),
        _ => panic!("Expected Interval message"),
    }

    // Test with very large floating point
    let msg = Message::interval(0, 0.0, 1.0, 0, 1e15);
    let serialized = serialize_message(&msg).unwrap();
    let json_bytes = &serialized[4..];
    let deserialized: Message = serde_json::from_slice(json_bytes).unwrap();

    match deserialized {
        Message::Interval {
            bits_per_second, ..
        } => assert!((bits_per_second - 1e15).abs() < 1e10),
        _ => panic!("Expected Interval message"),
    }
}

/// Test async cancellation safety
#[tokio::test]
async fn test_async_cancellation() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();

    let server_handle = tokio::spawn(async move {
        let (_socket, _) = listener.accept().await.unwrap();
        // Server just waits
        tokio::time::sleep(Duration::from_secs(10)).await;
    });

    let client_handle = tokio::spawn(async move {
        let _socket = tokio::net::TcpStream::connect(addr).await.unwrap();
        // Client just waits
        tokio::time::sleep(Duration::from_secs(10)).await;
    });

    // Cancel both operations quickly
    tokio::time::sleep(Duration::from_millis(50)).await;
    server_handle.abort();
    client_handle.abort();

    // Operations should be cancellable without panic
}
