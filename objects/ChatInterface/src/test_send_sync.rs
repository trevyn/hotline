use crate::{AnthropicClient, ChatInterface, Rect, TextArea};
use hotline::ObjectHandle;
use std::sync::Arc;
use std::thread;

fn assert_send<T: Send>() {}
fn assert_sync<T: Sync>() {}

#[test]
fn test_send_sync() {
    assert_send::<ChatInterface>();
    assert_sync::<ChatInterface>();

    // Check proxy types
    assert_send::<TextArea>();
    assert_sync::<TextArea>();
    assert_send::<Rect>();
    assert_sync::<Rect>();
    assert_send::<AnthropicClient>();
    assert_sync::<AnthropicClient>();

    // Check ObjectHandle
    assert_send::<ObjectHandle>();
    assert_sync::<ObjectHandle>();
}

#[test]
fn test_actual_thread_usage() {
    // Try to use ChatInterface across threads
    let chat = Arc::new(std::sync::Mutex::new(ChatInterface::default()));

    let chat_clone = chat.clone();
    let handle = thread::spawn(move || {
        let mut chat_guard = chat_clone.lock().unwrap();
        chat_guard.initialize();
    });

    handle.join().unwrap();

    // Try with proxy types
    let text_area = Arc::new(TextArea::new());
    let text_area_clone = text_area.clone();

    let handle = thread::spawn(move || {
        let _ta = text_area_clone;
    });

    handle.join().unwrap();
}
