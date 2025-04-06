#[cfg(test)]
mod tests {
    use super::*;
    use crate::connection::{SinkAdapter, StreamAdapter};
    use crate::message::{JointMessage, JointMessageMethod};
    use crate::response::Response;
    use async_trait::async_trait;
    use std::sync::{Arc, Mutex};

    // Mock implementation of SinkAdapter for testing
    struct MockSink {
        responses: Arc<Mutex<Vec<Response>>>,
    }

    #[async_trait]
    impl SinkAdapter for MockSink {
        async fn send(
            &mut self,
            response: Response,
        ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
            self.responses.lock().unwrap().push(response);
            Ok(())
        }
    }

    // Mock implementation of StreamAdapter for testing
    struct MockStream {
        messages: Vec<JointMessage>,
        index: usize,
    }

    #[async_trait]
    impl StreamAdapter for MockStream {
        async fn next(&mut self) -> Result<JointMessage, Box<dyn std::error::Error + Send + Sync>> {
            if self.index < self.messages.len() {
                let message = self.messages[self.index].clone();
                self.index += 1;
                Ok(message)
            } else {
                Err("End of stream".into())
            }
        }
    }

    #[tokio::test]
    async fn test_sink_adapter() {
        // Create a mock sink
        let responses = Arc::new(Mutex::new(Vec::new()));
        let mut sink = MockSink {
            responses: responses.clone(),
        };

        // Test sending a response
        let response = Response::RoomCreated(123);
        let result = sink.send(response.clone()).await;
        assert!(result.is_ok());

        // Verify the response was stored
        let stored_responses = responses.lock().unwrap();
        assert_eq!(stored_responses.len(), 1);

        // Match on the stored response type
        match &stored_responses[0] {
            Response::RoomCreated(id) => assert_eq!(*id, 123),
            _ => panic!("Expected RoomCreated response"),
        }
    }

    #[tokio::test]
    async fn test_stream_adapter() {
        // Create messages for testing
        let messages = vec![
            JointMessage {
                client_token: "1".to_string(),
                message: JointMessageMethod::Create,
            },
            JointMessage {
                client_token: "1".to_string(),
                message: JointMessageMethod::Join(42),
            },
            JointMessage {
                client_token: "1".to_string(),
                message: JointMessageMethod::Leave,
            },
        ];

        // Create a mock stream
        let mut stream = MockStream {
            messages: messages.clone(),
            index: 0,
        };

        // Test reading messages
        let message1 = stream.next().await.unwrap();
        assert!(matches!(message1.message, JointMessageMethod::Create));

        let message2 = stream.next().await.unwrap();
        if let JointMessageMethod::Join(room_id) = message2.message {
            assert_eq!(room_id, 42);
        } else {
            panic!("Expected Join message");
        }

        let message3 = stream.next().await.unwrap();
        assert!(matches!(message3.message, JointMessageMethod::Leave));

        // Test end of stream behavior
        let result = stream.next().await;
        assert!(result.is_err());
    }
}
