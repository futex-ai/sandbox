//! Unit tests for incremental Connect response framing.

use base64::{Engine, engine::general_purpose::STANDARD};

use crate::E2bAdapterError;

use super::{
    ConnectFrame, DecodedFrameBatch, FrameDecoder, ProcessDataChannel, ProcessEvent,
    decode_end_stream, decode_event, encode_frame,
};

#[test]
fn fragmented_and_coalesced_frames_decode_incrementally() {
    let first = encode_frame(br#"{"event":{"start":{"pid":42}}}"#).expect("first frame");
    let second = encode_frame(
        format!(
            r#"{{"event":{{"data":{{"pty":"{}"}}}}}}"#,
            STANDARD.encode("hello")
        )
        .as_bytes(),
    )
    .expect("second frame");
    let split = first.len() - 2;
    let mut decoder = FrameDecoder::new(1024);

    assert!(successful(decoder.push(&first[..split])).is_empty());
    let decoded = successful(decoder.push(&[first[split..].to_vec(), second].concat()));

    assert_eq!(decoded.len(), 2);
    assert_eq!(
        decode_event(&decoded[0].payload).expect("start"),
        ProcessEvent::Start(42)
    );
    assert_eq!(
        decode_event(&decoded[1].payload).expect("data"),
        ProcessEvent::Data {
            channel: ProcessDataChannel::Pty,
            bytes: b"hello".to_vec(),
        }
    );
}

#[test]
fn stdout_and_stderr_data_events_preserve_their_channel() {
    let stdout = format!(
        r#"{{"event":{{"data":{{"stdout":"{}"}}}}}}"#,
        STANDARD.encode("out")
    );
    let stderr = format!(
        r#"{{"event":{{"data":{{"stderr":"{}"}}}}}}"#,
        STANDARD.encode("err")
    );

    assert_eq!(
        decode_event(stdout.as_bytes()).expect("stdout"),
        ProcessEvent::Data {
            channel: ProcessDataChannel::Stdout,
            bytes: b"out".to_vec(),
        }
    );
    assert_eq!(
        decode_event(stderr.as_bytes()).expect("stderr"),
        ProcessEvent::Data {
            channel: ProcessDataChannel::Stderr,
            bytes: b"err".to_vec(),
        }
    );
}

#[test]
fn exit_keepalive_and_end_stream_events_are_preserved() {
    let mut decoder = FrameDecoder::new(1024);
    let end = frame(0, br#"{"event":{"end":{"exitCode":7,"exited":true}}}"#);
    let keepalive = frame(2, br#"{"event":{"keepalive":{}}}"#);

    let decoded = successful(decoder.push(&[end, keepalive].concat()));

    assert_eq!(
        decode_event(&decoded[0].payload).expect("end"),
        ProcessEvent::End {
            exit_code: 7,
            exited: true,
        }
    );
    assert!(decoded[1].end_stream);
    assert_eq!(
        decode_event(&decoded[1].payload).expect("keepalive"),
        ProcessEvent::KeepAlive
    );
}

#[test]
fn protobuf_default_end_fields_decode_as_zero_and_false() {
    let exited = decode_event(br#"{"event":{"end":{"exited":true,"status":"exit status 0"}}}"#)
        .expect("zero exit event");
    let running = decode_event(br#"{"event":{"end":{"status":"still running"}}}"#)
        .expect("default end event");

    assert_eq!(
        exited,
        ProcessEvent::End {
            exit_code: 0,
            exited: true,
        }
    );
    assert_eq!(
        running,
        ProcessEvent::End {
            exit_code: 0,
            exited: false,
        }
    );
}

#[test]
fn malformed_compressed_base64_and_json_frames_fail_closed() {
    let mut decoder = FrameDecoder::new(1024);
    assert!(matches!(
        decoder.push(&frame(1, b"{}")).terminal_error,
        Some(E2bAdapterError::MalformedFrame)
    ));
    let mut reserved = FrameDecoder::new(1024);
    assert!(matches!(
        reserved.push(&frame(4, b"{}")).terminal_error,
        Some(E2bAdapterError::MalformedFrame)
    ));
    assert!(matches!(
        decode_event(br#"{"event":{"data":{"pty":"%%%"}}}"#),
        Err(E2bAdapterError::MalformedFrame)
    ));
    assert!(matches!(
        decode_event(b"not-json"),
        Err(E2bAdapterError::MalformedFrame)
    ));
    assert!(matches!(
        decode_event(br#"{"event":{"data":{}}}"#),
        Err(E2bAdapterError::MalformedFrame)
    ));
    assert!(matches!(
        decode_event(br#"{"event":{"data":{"stdout":"b3V0","stderr":"ZXJy"}}}"#),
        Err(E2bAdapterError::MalformedFrame)
    ));
    assert!(matches!(
        decode_event(br#"{"event":{"start":{"pid":1},"keepalive":{}}}"#),
        Err(E2bAdapterError::MalformedFrame)
    ));
}

#[test]
fn zero_start_pid_is_malformed() {
    assert!(matches!(
        decode_event(br#"{"event":{"start":{"pid":0}}}"#),
        Err(E2bAdapterError::MalformedFrame)
    ));
}

#[test]
fn end_stream_decoder_accepts_success_and_lenient_error_shapes() {
    assert!(decode_end_stream(b"{}").is_ok());
    assert!(decode_end_stream(br#"{"error":null,"metadata":{"trace":["value"]}}"#).is_ok());
    assert!(matches!(
        decode_end_stream(br#"{"error":{"message":"failed","extension":true}}"#),
        Err(E2bAdapterError::Unavailable)
    ));
}

#[test]
fn malformed_end_stream_payloads_fail_closed() {
    assert!(matches!(
        decode_end_stream(b"not-json"),
        Err(E2bAdapterError::MalformedFrame)
    ));
    assert!(matches!(
        decode_end_stream(br#"{"error":"failed"}"#),
        Err(E2bAdapterError::MalformedFrame)
    ));
}

#[test]
fn complete_frames_are_preserved_before_a_terminal_decode_error() {
    let start = encode_frame(br#"{"event":{"start":{"pid":42}}}"#).expect("start frame");
    let mut decoder = FrameDecoder::new(1024);

    let decoded = decoder.push(&[start, frame(4, b"")].concat());

    assert_eq!(decoded.frames.len(), 1);
    assert_eq!(
        decode_event(&decoded.frames[0].payload).expect("start"),
        ProcessEvent::Start(42)
    );
    assert!(matches!(
        decoded.terminal_error,
        Some(E2bAdapterError::MalformedFrame)
    ));
}

#[test]
fn bytes_after_an_end_stream_frame_are_rejected() {
    let trailer = frame(2, b"{}");
    let extra = encode_frame(br#"{"event":{"keepalive":{}}}"#).expect("extra frame");
    let mut decoder = FrameDecoder::new(1024);

    let decoded = decoder.push(&[trailer, extra.clone()].concat());

    assert_eq!(decoded.frames.len(), 1);
    assert!(decoded.frames[0].end_stream);
    assert!(matches!(
        decoded.terminal_error,
        Some(E2bAdapterError::MalformedFrame)
    ));
    assert!(matches!(
        decoder.push(&extra).terminal_error,
        Some(E2bAdapterError::MalformedFrame)
    ));
}

#[test]
fn announced_oversized_frames_are_rejected() {
    let mut decoder = FrameDecoder::new(3);
    assert!(matches!(
        decoder.push(&frame(0, b"four")).terminal_error,
        Some(E2bAdapterError::ResponseTooLarge)
    ));
}

#[test]
fn oversized_fragments_are_rejected_before_they_are_buffered() {
    let maximum_payload = 1024;
    let mut fragment = frame(0, &vec![0; maximum_payload + 1]);
    fragment.extend(vec![0; 8 * 1024 * 1024]);
    let mut decoder = FrameDecoder::new(maximum_payload);

    let decoded = decoder.push(&fragment);

    assert!(matches!(
        decoded.terminal_error,
        Some(E2bAdapterError::ResponseTooLarge)
    ));
    assert!(decoder.buffer.len() <= 5);
}

fn successful(decoded: DecodedFrameBatch) -> Vec<ConnectFrame> {
    assert!(decoded.terminal_error.is_none());
    decoded.frames
}

fn frame(flags: u8, payload: &[u8]) -> Vec<u8> {
    let length = u32::try_from(payload.len()).expect("test frame length");
    let mut frame = vec![flags];
    frame.extend_from_slice(&length.to_be_bytes());
    frame.extend_from_slice(payload);
    frame
}
